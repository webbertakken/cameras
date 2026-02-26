import './CameraSidebar.css'

export function EmptyState() {
  return (
    <div role="status" aria-label="No cameras found" className="empty-state">
      <svg
        className="empty-state__icon"
        data-testid="camera-off-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        aria-hidden="true"
      >
        <path d="M15.75 10.5l4.72-4.72a.75.75 0 011.28.53v11.38a.75.75 0 01-1.28.53l-4.72-4.72M12 18.75H4.5a2.25 2.25 0 01-2.25-2.25V9m12.841 9.091L16.5 19.5m-1.409-.909l-7.5-7.5M3 3l18 18" />
      </svg>
      <p className="empty-state__text">No cameras found</p>
    </div>
  )
}
