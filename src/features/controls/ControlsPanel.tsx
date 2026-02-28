import { useCallback, useEffect, useReducer } from 'react'
import type { ControlDescriptor, ControlGroup, ResetResult } from '../../types/camera'
import { AccordionSection } from './AccordionSection'
import { ControlRenderer } from './ControlRenderer'
import './ControlsPanel.css'
import { ResetAllButton } from './ResetAllButton'
import { getCameraControls, resetCameraControl, setCameraControl } from './api'

/** Display labels for control groups. */
const GROUP_LABELS: Record<ControlGroup, string> = {
  image: 'Image',
  exposure: 'Exposure & white balance',
  focus: 'Focus & zoom',
  advanced: 'Advanced',
}

/** Canonical ordering for accordion groups. */
const GROUP_ORDER: ControlGroup[] = ['image', 'exposure', 'focus', 'advanced']

interface ControlsPanelProps {
  cameraId: string | null
  cameraName: string | null
}

interface ControlValue {
  value: number
  error?: string
}

interface PanelState {
  descriptors: ControlDescriptor[]
  values: Record<string, ControlValue>
  loading: boolean
}

type PanelAction =
  | { type: 'fetch_start' }
  | { type: 'fetch_success'; controls: ControlDescriptor[] }
  | { type: 'fetch_error' }
  | { type: 'set_value'; controlId: string; value: number }
  | { type: 'set_error'; controlId: string; value: number; error: string }
  | { type: 'reset_value'; controlId: string; value: number }
  | { type: 'reset_all'; results: ResetResult[] }

const initialState: PanelState = { descriptors: [], values: {}, loading: false }

function panelReducer(state: PanelState, action: PanelAction): PanelState {
  switch (action.type) {
    case 'fetch_start':
      return { descriptors: [], values: {}, loading: true }
    case 'fetch_success': {
      const values: Record<string, ControlValue> = {}
      for (const c of action.controls) {
        values[c.id] = { value: c.current }
      }
      return { descriptors: action.controls, values, loading: false }
    }
    case 'fetch_error':
      return { ...state, loading: false }
    case 'set_value':
      return {
        ...state,
        values: { ...state.values, [action.controlId]: { value: action.value } },
      }
    case 'set_error':
      return {
        ...state,
        values: {
          ...state.values,
          [action.controlId]: { value: action.value, error: action.error },
        },
      }
    case 'reset_value':
      return {
        ...state,
        values: { ...state.values, [action.controlId]: { value: action.value } },
      }
    case 'reset_all': {
      const values: Record<string, ControlValue> = { ...state.values }
      for (const r of action.results) {
        values[r.controlId] = { value: r.value }
      }
      return { ...state, values }
    }
  }
}

export function ControlsPanel({ cameraId, cameraName }: ControlsPanelProps) {
  const [{ descriptors, values, loading }, dispatch] = useReducer(panelReducer, initialState)

  useEffect(() => {
    if (!cameraId) return

    let cancelled = false
    dispatch({ type: 'fetch_start' })

    getCameraControls(cameraId).then(
      (controls) => {
        if (cancelled) return
        dispatch({ type: 'fetch_success', controls })
      },
      () => {
        if (cancelled) return
        dispatch({ type: 'fetch_error' })
      },
    )

    return () => {
      cancelled = true
    }
  }, [cameraId])

  const handleChange = useCallback(
    (controlId: string, newValue: number) => {
      if (!cameraId || !cameraName) return

      const previousValue = values[controlId]?.value

      dispatch({ type: 'set_value', controlId, value: newValue })

      setCameraControl(cameraId, controlId, newValue, cameraName).catch((err: unknown) => {
        const message = err instanceof Error ? err.message : 'Control rejected by hardware'
        dispatch({
          type: 'set_error',
          controlId,
          value: previousValue ?? newValue,
          error: message,
        })
      })
    },
    [cameraId, cameraName, values],
  )

  const handleResetAll = useCallback((results: ResetResult[]) => {
    dispatch({ type: 'reset_all', results })
  }, [])

  const handleReset = useCallback(
    (controlId: string) => {
      if (!cameraId) return

      resetCameraControl(cameraId, controlId).then(
        (defaultValue) => {
          dispatch({ type: 'reset_value', controlId, value: defaultValue })
        },
        (err: unknown) => {
          const message = err instanceof Error ? err.message : 'Reset failed'
          dispatch({
            type: 'set_error',
            controlId,
            value: values[controlId]?.value ?? 0,
            error: message,
          })
        },
      )
    },
    [cameraId, values],
  )

  if (!cameraId || !cameraName) {
    return (
      <section aria-label="Camera controls" className="controls-panel">
        <p className="controls-panel__empty">Select a camera to view its controls</p>
      </section>
    )
  }

  if (loading) {
    return (
      <section aria-label="Camera controls" className="controls-panel">
        <div aria-label="Loading controls" className="controls-panel__loading">
          <div className="controls-panel__skeleton" />
          <div className="controls-panel__skeleton" />
          <div className="controls-panel__skeleton" />
        </div>
      </section>
    )
  }

  if (descriptors.length === 0) {
    return (
      <section aria-label="Camera controls" className="controls-panel">
        <p className="controls-panel__empty">No adjustable controls</p>
      </section>
    )
  }

  // Group controls by their group field
  const groups = new Map<ControlGroup, ControlDescriptor[]>()
  for (const desc of descriptors) {
    const list = groups.get(desc.group) ?? []
    list.push(desc)
    groups.set(desc.group, list)
  }

  // Determine if all controls fit in a single section
  const totalControls = descriptors.length
  const expandAll = totalControls <= 3

  return (
    <section aria-label="Camera controls" role="region" className="controls-panel">
      {GROUP_ORDER.filter((g) => groups.has(g)).map((group, index) => {
        const controls = groups.get(group) ?? []
        const allUnsupported = controls.every((c) => !c.supported)
        const defaultExpanded = expandAll || index === 0 ? !allUnsupported : false

        return (
          <AccordionSection
            key={group}
            label={GROUP_LABELS[group]}
            sectionId={group}
            defaultExpanded={defaultExpanded}
          >
            {controls.map((desc) => (
              <ControlRenderer
                key={desc.id}
                descriptor={desc}
                value={values[desc.id]?.value ?? desc.current}
                cameraName={cameraName}
                onChange={(v) => handleChange(desc.id, v)}
                onReset={() => handleReset(desc.id)}
                error={values[desc.id]?.error}
              />
            ))}
          </AccordionSection>
        )
      })}
      <ResetAllButton cameraId={cameraId} cameraName={cameraName} onReset={handleResetAll} />
    </section>
  )
}
