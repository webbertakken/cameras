import './PreviewCanvas.css'

interface PreviewCanvasProps {
  /** Blob URL or null when no frame available. */
  frameSrc: string | null
  /** Whether the preview is loading (camera selected but no frames yet). */
  isLoading: boolean
  /** Error message when preview fails. */
  error?: string | null
}

/** Renders a camera preview frame, scaled to fit its container. */
export function PreviewCanvas({ frameSrc, isLoading, error }: PreviewCanvasProps) {
  if (error) {
    return (
      <div className="preview-canvas preview-canvas--empty" role="img" aria-label="Preview error">
        <p className="preview-canvas__error-title">Preview unavailable</p>
        <p className="preview-canvas__error-detail">{error}</p>
      </div>
    )
  }

  if (isLoading && !frameSrc) {
    return (
      <div className="preview-canvas preview-canvas--empty" role="img" aria-label="Loading preview">
        <div className="preview-canvas__spinner" />
        <span className="preview-canvas__placeholder">Starting preview...</span>
      </div>
    )
  }

  if (!frameSrc) {
    return (
      <div
        className="preview-canvas preview-canvas--empty"
        role="img"
        aria-label="No preview available"
      >
        <span className="preview-canvas__placeholder">No preview</span>
      </div>
    )
  }

  return (
    <div className="preview-canvas">
      <img className="preview-canvas__image" src={frameSrc} alt="Camera preview" role="img" />
    </div>
  )
}
