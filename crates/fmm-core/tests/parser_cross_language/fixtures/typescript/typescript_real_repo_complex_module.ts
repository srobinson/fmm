
import { EventEmitter } from 'events';
import { Logger } from 'winston';
import { Config } from './config';
import { Database } from '../database';

export interface ConnectionOptions {
    host: string;
    port: number;
    ssl: boolean;
}

export class ConnectionManager extends EventEmitter {
    private logger: Logger;
    private config: Config;

    constructor(config: Config) {
        super();
        this.config = config;
        this.logger = new Logger();
    }

    async connect(options: ConnectionOptions): Promise<void> {
        this.emit('connecting', options);
    }

    async disconnect(): Promise<void> {
        this.emit('disconnected');
    }
}

export function createConnection(config: Config): ConnectionManager {
    return new ConnectionManager(config);
}

export const DEFAULT_PORT = 5432;
