import './PreviewCanvas.css'

interface PreviewCanvasProps {
  /** Base64 JPEG data URI (data:image/jpeg;base64,...) or null when no frame. */
  frameSrc: string | null
}

/** Renders a camera preview frame, scaled to fit its container. */
export function PreviewCanvas({ frameSrc }: PreviewCanvasProps) {
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
