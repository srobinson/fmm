package com.example.api

import io.ktor.server.application.Application
import io.ktor.server.routing.routing

data class ApiResponse<T>(val data: T?, val message: String)

interface UserService {
    fun findById(id: String): Any?
    fun save(user: Any): Boolean
}

class UserServiceImpl : UserService {
    override fun findById(id: String): Any? = null
    override fun save(user: Any): Boolean = true
}

fun configureRouting(app: Application) {}

val API_VERSION = "v1"

typealias Handler = (Any) -> ApiResponse<Any>
