import { config, validateConfig } from './config/app';
import { getAuthRoutes } from './api/routes/auth';
import { getUserRoutes } from './api/routes/users';
import { rateLimiter } from './middleware/rateLimit';

export function createApp() {
  const configErrors = validateConfig(config);
  if (configErrors.length > 0) {
    console.error('Configuration errors:', configErrors);
    process.exit(1);
  }

  const routes = [...getAuthRoutes(), ...getUserRoutes()];
  const globalMiddleware = [rateLimiter(config.rateLimitMax, config.rateLimitWindow)];

  return {
    routes,
    globalMiddleware,
    config,
    start() {
      console.log(`Server starting on port ${config.port}`);
      console.log(`${routes.length} routes registered`);
    },
  };
}

if (require.main === module) {
  const app = createApp();
  app.start();
}
