import type { ButtonHTMLAttributes, HTMLAttributes, InputHTMLAttributes, SelectHTMLAttributes } from 'react';
import { AeroIcon } from './Icon';
import type { IconName } from './Icon';

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: 'standard' | 'accent' | 'toolbar';
};

export function AeroButton({ variant = 'standard', className = '', type = 'button', ...props }: ButtonProps) {
  return <button type={type} className={`aero-button aero-button--${variant} ${className}`} {...props} />;
}

export function AeroIconButton({ icon, label, ...props }: Omit<ButtonProps, 'children' | 'aria-label' | 'title'> & { icon: IconName; label: string }) {
  return <AeroButton {...props} title={label} aria-label={label} className={`aero-icon-button ${props.className ?? ''}`}><AeroIcon name={icon} size={18} /></AeroButton>;
}

export function AeroSelect({ className = '', ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className={`aero-select ${className}`} {...props} />;
}

export function AeroInput({ className = '', ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={`aero-input ${className}`} {...props} />;
}

export function AeroRange({ className = '', ...props }: Omit<InputHTMLAttributes<HTMLInputElement>, 'type'>) {
  return <input {...props} type="range" className={`aero-range ${className}`} />;
}

export function AeroToolbar({ className = '', ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`aero-toolbar ${className}`} {...props} />;
}

export function AeroGlass({ className = '', ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`aero-glass ${className}`} {...props} />;
}
