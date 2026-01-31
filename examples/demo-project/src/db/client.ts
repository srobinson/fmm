import { Pool, QueryResult } from 'pg';
import { AppConfig } from '../config';

export class DatabaseClient {
  private pool: Pool;

  constructor(config: AppConfig) {
    this.pool = new Pool({ connectionString: config.dbUrl });
  }

  async query<T>(sql: string, params?: unknown[]): Promise<QueryResult<T>> {
    return this.pool.query(sql, params);
  }

  async close(): Promise<void> {
    await this.pool.end();
  }
}

export async function createClient(config: AppConfig): Promise<DatabaseClient> {
  const client = new DatabaseClient(config);
  return client;
}
