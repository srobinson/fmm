// --- FMM ---
// fmm: v0.2
// file: services/email.ts
// exports: [EmailMessage, clearSentEmails, getSentEmails, sendEmail, sendPasswordResetEmail, sendWelcomeEmail]
// loc: 35
// modified: 2026-01-29
// ---

export interface EmailMessage {
  to: string;
  subject: string;
  body: string;
}

const sentEmails: EmailMessage[] = [];

export async function sendWelcomeEmail(email: string, name: string): Promise<void> {
  await sendEmail({
    to: email,
    subject: 'Welcome!',
    body: `Hello ${name}, welcome to the platform!`,
  });
}

export async function sendPasswordResetEmail(email: string, resetCode: string): Promise<void> {
  await sendEmail({
    to: email,
    subject: 'Password Reset',
    body: `Your reset code: ${resetCode}. Valid for 15 minutes.`,
  });
}

export async function sendEmail(message: EmailMessage): Promise<void> {
  sentEmails.push(message);
}

export function getSentEmails(): EmailMessage[] {
  return [...sentEmails];
}

export function clearSentEmails(): void {
  sentEmails.length = 0;
}
