import type { ButtonHTMLAttributes, HTMLAttributes, SelectHTMLAttributes } from 'react';

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: 'standard' | 'accent' | 'toolbar';
};

export function AeroButton({ variant = 'standard', className = '', type = 'button', ...props }: ButtonProps) {
  return <button type={type} className={`aero-button aero-button--${variant} ${className}`} {...props} />;
}

export function AeroSelect({ className = '', ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className={`aero-select ${className}`} {...props} />;
}

export function AeroToolbar({ className = '', ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`aero-toolbar ${className}`} {...props} />;
}

export function AeroPanel({ className = '', ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`aero-panel ${className}`} {...props} />;
}
