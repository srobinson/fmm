
import { readFileSync } from 'fs';
import { join } from 'path';

const CONFIG_PATH = join(__dirname, 'defaults.json');

function loadDefaults(): Record<string, unknown> {
    const raw = readFileSync(CONFIG_PATH, 'utf-8');
    return JSON.parse(raw);
}

function mergeConfig(base: Record<string, unknown>, overrides: Record<string, unknown>): Record<string, unknown> {
    return { ...base, ...overrides };
}
