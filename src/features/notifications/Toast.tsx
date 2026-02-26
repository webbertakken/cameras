import type { ToastType } from './useToast'
import './Toast.css'

interface ToastProps {
  id: string
  message: string
  type: ToastType
  onDismiss: (id: string) => void
}

export function Toast({ id, message, type, onDismiss }: ToastProps) {
  return (
    <div className={`toast toast--${type}`} role="status" aria-live="polite">
      <span className="toast__message">{message}</span>
      <button
        className="toast__dismiss"
        aria-label="Dismiss notification"
        onClick={() => onDismiss(id)}
      >
        &times;
      </button>
    </div>
  )
}
