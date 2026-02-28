import { useCallback, useEffect, useId, useRef } from 'react'
import { createPortal } from 'react-dom'
import './ConfirmModal.css'

interface ConfirmModalProps {
  open: boolean
  title: string
  message: string
  confirmLabel?: string
  cancelLabel?: string
  confirmDisabled?: boolean
  onConfirm: () => void
  onCancel: () => void
}

export function ConfirmModal({
  open,
  title,
  message,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  confirmDisabled = false,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  const titleId = useId()
  const cancelRef = useRef<HTMLButtonElement>(null)
  const confirmRef = useRef<HTMLButtonElement>(null)
  const previousFocusRef = useRef<Element | null>(null)
  const onCancelRef = useRef(onCancel)
  useEffect(() => {
    onCancelRef.current = onCancel
  }, [onCancel])

  useEffect(() => {
    if (open) {
      previousFocusRef.current = document.activeElement
      document.body.style.overflow = 'hidden'
      // Focus cancel button after portal renders
      requestAnimationFrame(() => {
        cancelRef.current?.focus()
      })

      const handleEscape = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
          onCancelRef.current()
        }
      }
      document.addEventListener('keydown', handleEscape)

      return () => {
        document.removeEventListener('keydown', handleEscape)
        document.body.style.overflow = ''
      }
    }

    document.body.style.overflow = ''
    if (previousFocusRef.current instanceof HTMLElement) {
      previousFocusRef.current.focus()
    }
    return undefined
  }, [open])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    // Focus trap: Tab/Shift+Tab cycles between buttons
    if (e.key === 'Tab') {
      e.preventDefault()
      if (document.activeElement === cancelRef.current) {
        confirmRef.current?.focus()
      } else {
        cancelRef.current?.focus()
      }
    }
  }, [])

  const handleOverlayClick = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === e.currentTarget) {
        onCancel()
      }
    },
    [onCancel],
  )

  if (!open) return null

  return createPortal(
    <div
      className="confirm-modal__overlay"
      onMouseDown={handleOverlayClick}
      onKeyDown={handleKeyDown}
    >
      <div className="confirm-modal" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <h2 id={titleId} className="confirm-modal__title">
          {title}
        </h2>
        <p className="confirm-modal__message">{message}</p>
        <div className="confirm-modal__actions">
          <button
            ref={cancelRef}
            type="button"
            className="confirm-modal__btn confirm-modal__btn--cancel"
            onClick={onCancel}
            disabled={confirmDisabled}
          >
            {cancelLabel}
          </button>
          <button
            ref={confirmRef}
            type="button"
            className="confirm-modal__btn confirm-modal__btn--confirm"
            onClick={onConfirm}
            disabled={confirmDisabled}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  )
}
