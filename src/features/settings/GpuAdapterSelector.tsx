import { useCallback, useEffect, useState } from 'react'
import type { GpuAdapterInfo } from '../../types/gpu'
import { useToastStore } from '../notifications/useToast'
import { getActiveGpu, listGpuAdapters, setGpuAdapter } from './gpu-api'
import './GpuAdapterSelector.css'

/** Value representing CPU-only mode in the dropdown. */
const CPU_ONLY_VALUE = 'cpu'

/**
 * Format an adapter for display: "Name (Backend)".
 *
 * Device type is included only when it provides useful disambiguation
 * (e.g. integrated vs discrete on the same machine).
 */
function formatAdapter(adapter: GpuAdapterInfo): string {
  return `${adapter.name} (${adapter.backend})`
}

/**
 * Dropdown for selecting the GPU adapter used for frame processing.
 *
 * Includes a "CPU only" option to disable GPU acceleration entirely.
 * Falls back gracefully when no adapters are available.
 */
export function GpuAdapterSelector() {
  const [adapters, setAdapters] = useState<GpuAdapterInfo[]>([])
  const [activeIndex, setActiveIndex] = useState<number | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false

    Promise.all([listGpuAdapters(), getActiveGpu()])
      .then(([adapterList, active]) => {
        if (cancelled) return
        setAdapters(adapterList)
        setActiveIndex(active?.index ?? null)
      })
      .catch((err: unknown) => {
        if (cancelled) return
        const message = err instanceof Error ? err.message : String(err)
        useToastStore.getState().addToast(`Failed to load GPU adapters: ${message}`, 'error')
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [])

  const handleChange = useCallback((event: React.ChangeEvent<HTMLSelectElement>) => {
    const value = event.target.value
    const newIndex = value === CPU_ONLY_VALUE ? null : Number(value)

    setActiveIndex(newIndex)

    setGpuAdapter(newIndex)
      .then((name) => {
        if (name) {
          useToastStore.getState().addToast(`GPU switched to ${name}`, 'success')
        } else if (newIndex === null) {
          useToastStore.getState().addToast('GPU acceleration disabled', 'success')
        } else {
          useToastStore.getState().addToast('GPU adapter unavailable — using CPU', 'info')
          setActiveIndex(null)
        }
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : String(err)
        useToastStore.getState().addToast(`Failed to switch GPU: ${message}`, 'error')
      })
  }, [])

  if (loading) {
    return (
      <div className="gpu-selector">
        <label className="gpu-selector__label">Processing</label>
        <div className="gpu-selector__skeleton" />
      </div>
    )
  }

  const selectValue = activeIndex !== null ? String(activeIndex) : CPU_ONLY_VALUE

  return (
    <div className="gpu-selector">
      <label className="gpu-selector__label" htmlFor="gpu-adapter-select">
        Processing
      </label>
      <select
        id="gpu-adapter-select"
        className="gpu-selector__select"
        value={selectValue}
        onChange={handleChange}
        aria-label="Select GPU adapter for frame processing"
      >
        <option value={CPU_ONLY_VALUE}>CPU only</option>
        {adapters.map((adapter) => (
          <option key={adapter.index} value={String(adapter.index)}>
            {formatAdapter(adapter)}
          </option>
        ))}
      </select>
    </div>
  )
}
