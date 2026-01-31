import express from 'express';
import { loadConfig } from './config';
import { createClient } from './db/client';
import { createRouter } from './api/routes';
import { errorHandler } from './api/errors';

async function main(): Promise<void> {
  const config = loadConfig();
  const db = await createClient(config);
  const app = express();

  app.use(express.json());
  app.use('/api', createRouter(db));
  app.use(errorHandler);

  app.listen(config.port, () => {
    console.log(`Server running on port ${config.port}`);
  });
}

main().catch(console.error);
