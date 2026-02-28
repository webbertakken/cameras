import { useCallback, useState } from 'react'
import type { ResetResult } from '../../types/camera'
import { useToastStore } from '../notifications/useToast'
import { ConfirmModal } from './ConfirmModal'
import './ResetAllButton.css'
import { resetAllToDefaults } from './api'

interface ResetAllButtonProps {
  cameraId: string
  cameraName: string
  onReset: (results: ResetResult[]) => void
}

export function ResetAllButton({ cameraId, cameraName, onReset }: ResetAllButtonProps) {
  const [modalOpen, setModalOpen] = useState(false)
  const [resetting, setResetting] = useState(false)

  const handleConfirm = useCallback(async () => {
    setResetting(true)
    try {
      const results = await resetAllToDefaults(cameraId)
      onReset(results)
      useToastStore.getState().addToast('All controls reset to defaults', 'success')
      setModalOpen(false)
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Reset failed'
      useToastStore.getState().addToast(message, 'error')
    } finally {
      setResetting(false)
    }
  }, [cameraId, onReset])

  const handleCancel = useCallback(() => {
    setModalOpen(false)
  }, [])

  return (
    <>
      <div className="reset-all-button">
        <button type="button" className="reset-all-button__btn" onClick={() => setModalOpen(true)}>
          Reset all to defaults
        </button>
      </div>
      <ConfirmModal
        open={modalOpen}
        title="Reset to defaults?"
        message={`All controls for ${cameraName} will be reset to their hardware defaults. This cannot be undone.`}
        confirmLabel={resetting ? 'Resetting...' : 'Reset'}
        cancelLabel="Cancel"
        onConfirm={handleConfirm}
        onCancel={handleCancel}
      />
    </>
  )
}
