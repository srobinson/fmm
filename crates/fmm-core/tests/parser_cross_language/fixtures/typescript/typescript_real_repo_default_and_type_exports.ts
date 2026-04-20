
import React from 'react';
import { useStore } from './store';
import type { Theme } from './theme';

export type AppProps = {
    title: string;
    theme: Theme;
};

export interface AppState {
    loading: boolean;
    error: string | null;
}

export const APP_VERSION = "2.0.0";

export default function App({ title, theme }: AppProps) {
    const store = useStore();
    return React.createElement('div', { className: theme }, title);
}
