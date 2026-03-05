import { EventEmitter } from 'events';
import { readFile } from 'fs/promises';
import { Helper } from './helper';
import type { BaseConfig } from '../config';

export interface AppConfig {
    port: number;
    debug: boolean;
    name: string;
}

export type Handler<T> = (event: T) => void;

export enum Status {
    Active = 'active',
    Inactive = 'inactive',
    Pending = 'pending',
}

export const DEFAULT_PORT = 3000;

export class AppService {
    private emitter: EventEmitter;

    constructor(private config: AppConfig) {
        this.emitter = new EventEmitter();
    }

    async start(): Promise<void> {
        this.emitter.emit('start');
    }

    stop(): void {
        this.emitter.removeAllListeners();
    }
}

export function createApp(config: AppConfig): AppService {
    return new AppService(config);
}

export default AppService;
