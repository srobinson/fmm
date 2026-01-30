use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fmm::parser::builtin::python::PythonParser;
use fmm::parser::builtin::rust::RustParser;
use fmm::parser::builtin::typescript::TypeScriptParser;
use fmm::parser::Parser;
use std::time::Duration;

const TYPESCRIPT_SOURCE: &str = r#"
import { Request, Response, NextFunction } from 'express';
import jwt from 'jsonwebtoken';
import { Redis } from 'ioredis';
import { Logger } from './logger';
import { Config } from './config';
import { UserService } from '../services/user';

export interface AuthPayload {
    userId: string;
    email: string;
    roles: string[];
    exp: number;
}

export interface SessionData {
    token: string;
    refreshToken: string;
    expiresAt: Date;
}

export class AuthMiddleware {
    private redis: Redis;
    private secret: string;

    constructor(redis: Redis, secret: string) {
        this.redis = redis;
        this.secret = secret;
    }

    async authenticate(req: Request, res: Response, next: NextFunction): Promise<void> {
        const token = req.headers.authorization?.replace('Bearer ', '');
        if (!token) {
            res.status(401).json({ error: 'No token provided' });
            return;
        }
        try {
            const payload = jwt.verify(token, this.secret) as AuthPayload;
            req.user = payload;
            next();
        } catch {
            res.status(401).json({ error: 'Invalid token' });
        }
    }

    async createSession(userId: string, email: string): Promise<SessionData> {
        const token = jwt.sign({ userId, email }, this.secret, { expiresIn: '1h' });
        const refreshToken = jwt.sign({ userId }, this.secret, { expiresIn: '7d' });
        await this.redis.set(`session:${userId}`, token, 'EX', 3600);
        return { token, refreshToken, expiresAt: new Date(Date.now() + 3600000) };
    }
}

export function validateEmail(email: string): boolean {
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

export const DEFAULT_EXPIRY = 3600;
"#;

const PYTHON_SOURCE: &str = r#"
"""Authentication module for the web application."""

import os
import hashlib
import hmac
from datetime import datetime, timedelta
from typing import Optional, Dict, Any

import jwt
import redis
from flask import request, jsonify, g
from .models import User, Session
from .config import settings
from ..utils import validate_email

__all__ = ["authenticate", "create_session", "AuthManager", "TokenPayload"]

MAX_RETRIES = 3
DEFAULT_EXPIRY = 3600


class TokenPayload:
    """JWT token payload structure."""

    def __init__(self, user_id: str, email: str, roles: list):
        self.user_id = user_id
        self.email = email
        self.roles = roles

    def to_dict(self) -> Dict[str, Any]:
        return {"user_id": self.user_id, "email": self.email, "roles": self.roles}


class AuthManager:
    """Manages authentication and session lifecycle."""

    def __init__(self, secret_key: str, redis_url: str):
        self._secret = secret_key
        self._redis = redis.from_url(redis_url)

    @staticmethod
    def hash_password(password: str) -> str:
        salt = os.urandom(32)
        return hashlib.pbkdf2_hmac("sha256", password.encode(), salt, 100000).hex()

    def verify_token(self, token: str) -> Optional[TokenPayload]:
        try:
            data = jwt.decode(token, self._secret, algorithms=["HS256"])
            return TokenPayload(**data)
        except jwt.InvalidTokenError:
            return None


def authenticate(token: str) -> bool:
    """Authenticate a request using JWT token."""
    if not token:
        return False
    try:
        jwt.decode(token, settings.SECRET_KEY, algorithms=["HS256"])
        return True
    except jwt.InvalidTokenError:
        return False


def create_session(user_id: str, ttl: int = DEFAULT_EXPIRY) -> str:
    """Create a new session token."""
    payload = {"user_id": user_id, "exp": datetime.utcnow() + timedelta(seconds=ttl)}
    return jwt.encode(payload, settings.SECRET_KEY, algorithm="HS256")


def _internal_validate(data: dict) -> bool:
    return "user_id" in data
"#;

const RUST_SOURCE: &str = r#"
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use crate::config::Settings;
use super::utils;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub secret: String,
    pub expiry_secs: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPayload {
    pub user_id: String,
    pub email: String,
    pub roles: Vec<String>,
    pub exp: u64,
}

pub enum AuthError {
    InvalidToken,
    Expired,
    Forbidden,
}

pub trait Authenticator: Send + Sync {
    fn verify(&self, token: &str) -> Result<TokenPayload>;
    fn create_token(&self, payload: &TokenPayload) -> Result<String>;
}

pub struct JwtAuth<'a> {
    config: &'a AuthConfig,
    cache: Arc<RwLock<HashMap<String, TokenPayload>>>,
}

impl<'a> JwtAuth<'a> {
    pub fn new(config: &'a AuthConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidToken => write!(f, "Invalid token"),
            AuthError::Expired => write!(f, "Token expired"),
            AuthError::Forbidden => write!(f, "Forbidden"),
        }
    }
}

pub fn validate_email(email: &str) -> bool {
    email.contains('@') && email.contains('.')
}

pub(crate) fn internal_hash(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

pub async fn refresh_token(token: &str) -> Result<String> {
    Ok(token.to_string())
}

fn private_helper() -> bool {
    true
}

pub fn process_batch(items: &[String]) -> Result<Vec<String>> {
    let results: Vec<String> = items.iter().map(|s| s.to_uppercase()).collect();
    let _raw = unsafe { std::ptr::null::<u8>().is_null() };
    Ok(results)
}
"#;

fn bench_parse_typescript(c: &mut Criterion) {
    c.bench_function("parse_typescript_single", |b| {
        let mut parser = TypeScriptParser::new().unwrap();
        b.iter(|| {
            parser.parse(black_box(TYPESCRIPT_SOURCE)).unwrap();
        });
    });
}

fn bench_parse_python(c: &mut Criterion) {
    c.bench_function("parse_python_single", |b| {
        let mut parser = PythonParser::new().unwrap();
        b.iter(|| {
            parser.parse(black_box(PYTHON_SOURCE)).unwrap();
        });
    });
}

fn bench_parse_rust(c: &mut Criterion) {
    c.bench_function("parse_rust_single", |b| {
        let mut parser = RustParser::new().unwrap();
        b.iter(|| {
            parser.parse(black_box(RUST_SOURCE)).unwrap();
        });
    });
}

fn bench_batch_100(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_100");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));
    group.bench_function("mixed_files", |b| {
        let mut ts_parser = TypeScriptParser::new().unwrap();
        let mut py_parser = PythonParser::new().unwrap();
        let mut rs_parser = RustParser::new().unwrap();

        b.iter(|| {
            for _ in 0..34 {
                ts_parser.parse(black_box(TYPESCRIPT_SOURCE)).unwrap();
            }
            for _ in 0..33 {
                py_parser.parse(black_box(PYTHON_SOURCE)).unwrap();
            }
            for _ in 0..33 {
                rs_parser.parse(black_box(RUST_SOURCE)).unwrap();
            }
        });
    });
    group.finish();
}

fn bench_batch_1000(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_1000");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));
    group.bench_function("mixed_files", |b| {
        let mut ts_parser = TypeScriptParser::new().unwrap();
        let mut py_parser = PythonParser::new().unwrap();
        let mut rs_parser = RustParser::new().unwrap();

        b.iter(|| {
            for _ in 0..334 {
                ts_parser.parse(black_box(TYPESCRIPT_SOURCE)).unwrap();
            }
            for _ in 0..333 {
                py_parser.parse(black_box(PYTHON_SOURCE)).unwrap();
            }
            for _ in 0..333 {
                rs_parser.parse(black_box(RUST_SOURCE)).unwrap();
            }
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_parse_typescript,
    bench_parse_python,
    bench_parse_rust,
    bench_batch_100,
    bench_batch_1000
);
criterion_main!(benches);
