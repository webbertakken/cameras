import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ControlDescriptor } from '../../types/camera'
import { ControlsPanel } from './ControlsPanel'

vi.mock('./api', () => ({
  getCameraControls: vi.fn(),
  setCameraControl: vi.fn(),
  resetCameraControl: vi.fn(),
}))

const { getCameraControls, setCameraControl, resetCameraControl } = await import('./api')
const mockGetControls = vi.mocked(getCameraControls)
const mockSetControl = vi.mocked(setCameraControl)
const mockResetControl = vi.mocked(resetCameraControl)

const brightness: ControlDescriptor = {
  id: 'brightness',
  name: 'Brightness',
  controlType: 'slider',
  group: 'image',
  min: 0,
  max: 255,
  step: 1,
  default: 128,
  current: 150,
  flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
  supported: true,
}

const contrast: ControlDescriptor = {
  ...brightness,
  id: 'contrast',
  name: 'Contrast',
  current: 100,
  default: 64,
}

const exposure: ControlDescriptor = {
  id: 'exposure',
  name: 'Exposure',
  controlType: 'slider',
  group: 'exposure',
  min: -11,
  max: -2,
  step: 1,
  default: -6,
  current: -6,
  flags: { supportsAuto: true, isAutoEnabled: true, isReadOnly: false },
  supported: true,
}

const pan: ControlDescriptor = {
  id: 'pan',
  name: 'Pan',
  controlType: 'slider',
  group: 'advanced',
  min: -180,
  max: 180,
  step: 1,
  default: 0,
  current: 0,
  flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
  supported: false,
}

const allControls = [brightness, contrast, exposure, pan]

describe('ControlsPanel', () => {
  beforeEach(() => {
    mockGetControls.mockReset()
    mockSetControl.mockReset()
    mockResetControl.mockReset()
  })

  // --- Loading ---

  it('fetches controls when selectedCameraId changes', async () => {
    mockGetControls.mockResolvedValue(allControls)
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)
    await waitFor(() => {
      expect(mockGetControls).toHaveBeenCalledWith('cam-1')
    })
  })

  it('shows loading state while fetching', () => {
    mockGetControls.mockReturnValue(new Promise(() => {})) // never resolves
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)
    expect(screen.getByLabelText('Loading controls')).toBeInTheDocument()
  })

  // --- Grouping ---

  it('groups controls by group field into accordion sections', async () => {
    mockGetControls.mockResolvedValue(allControls)
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)
    await waitFor(() => {
      expect(screen.getByText('Image')).toBeInTheDocument()
    })
    expect(screen.getByText('Exposure & white balance')).toBeInTheDocument()
    expect(screen.getByText('Advanced')).toBeInTheDocument()
  })

  it('expands "Image" group by default', async () => {
    mockGetControls.mockResolvedValue(allControls)
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)
    await waitFor(() => {
      expect(screen.getByText('Brightness')).toBeVisible()
    })
  })

  it('shows single expanded section when camera has <= 3 controls', async () => {
    mockGetControls.mockResolvedValue([brightness])
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)
    await waitFor(() => {
      expect(screen.getByText('Brightness')).toBeVisible()
    })
  })

  // --- Control interaction ---

  it('calls setCameraControl IPC on slider change', async () => {
    const user = userEvent.setup()
    mockGetControls.mockResolvedValue([brightness])
    mockSetControl.mockResolvedValue(undefined)
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)

    await waitFor(() => {
      expect(screen.getByRole('slider')).toBeInTheDocument()
    })

    // Click on the readout to enter direct input mode
    await user.click(screen.getByText('150'))
    const input = screen.getByRole('spinbutton')
    await user.clear(input)
    await user.type(input, '200')
    await user.keyboard('{Enter}')

    await waitFor(() => {
      expect(mockSetControl).toHaveBeenCalledWith('cam-1', 'brightness', 200)
    })
  })

  it('reverts slider on backend rejection', async () => {
    const user = userEvent.setup()
    mockGetControls.mockResolvedValue([brightness])
    mockSetControl.mockRejectedValue(new Error('Hardware rejected'))
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)

    await waitFor(() => {
      expect(screen.getByRole('slider')).toBeInTheDocument()
    })

    await user.click(screen.getByText('150'))
    const input = screen.getByRole('spinbutton')
    await user.clear(input)
    await user.type(input, '200')
    await user.keyboard('{Enter}')

    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument()
    })
  })

  it('calls resetCameraControl on reset button click', async () => {
    const user = userEvent.setup()
    mockGetControls.mockResolvedValue([brightness])
    mockResetControl.mockResolvedValue(128)
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /reset/i })).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: /reset/i }))

    await waitFor(() => {
      expect(mockResetControl).toHaveBeenCalledWith('cam-1', 'brightness')
    })
  })

  it('updates slider to returned default value after reset', async () => {
    const user = userEvent.setup()
    mockGetControls.mockResolvedValue([brightness])
    mockResetControl.mockResolvedValue(128)
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)

    await waitFor(() => {
      expect(screen.getByText('150')).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: /reset/i }))

    await waitFor(() => {
      expect(screen.getByText('128')).toBeInTheDocument()
    })
  })

  // --- Empty controls ---

  it('shows "No adjustable controls" when camera returns empty controls', async () => {
    mockGetControls.mockResolvedValue([])
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)
    await waitFor(() => {
      expect(screen.getByText(/no adjustable controls/i)).toBeInTheDocument()
    })
  })

  // --- Camera switching ---

  it('shows empty state when no camera is selected', () => {
    render(<ControlsPanel cameraId={null} cameraName={null} />)
    expect(screen.getByText(/select a camera/i)).toBeInTheDocument()
  })

  // --- Accessibility ---

  it('panel has aria-label "Camera controls"', async () => {
    mockGetControls.mockResolvedValue([brightness])
    render(<ControlsPanel cameraId="cam-1" cameraName="Test Cam" />)
    await waitFor(() => {
      expect(screen.getByRole('region', { name: 'Camera controls' })).toBeInTheDocument()
    })
  })
})
