import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { GpuAdapterInfo } from '../../types/gpu'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

const { invoke } = await import('@tauri-apps/api/core')
const mockInvoke = vi.mocked(invoke)

// Import after mocking
const { GpuAdapterSelector } = await import('./GpuAdapterSelector')

const vulkanAdapter: GpuAdapterInfo = {
  index: 0,
  name: 'NVIDIA GeForce RTX 3080',
  backend: 'Vulkan',
  deviceType: 'DiscreteGpu',
}

const dx12Adapter: GpuAdapterInfo = {
  index: 1,
  name: 'NVIDIA GeForce RTX 3080',
  backend: 'Dx12',
  deviceType: 'DiscreteGpu',
}

describe('GpuAdapterSelector', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
  })

  it('renders a loading skeleton initially', () => {
    mockInvoke.mockReturnValue(new Promise(() => {}))
    render(<GpuAdapterSelector />)
    expect(screen.getByText('Processing')).toBeInTheDocument()
  })

  it('renders a dropdown with adapters after loading', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_gpu_adapters') return [vulkanAdapter, dx12Adapter]
      if (cmd === 'get_active_gpu') return vulkanAdapter
      return null
    })

    render(<GpuAdapterSelector />)

    await waitFor(() => {
      expect(screen.getByRole('combobox')).toBeInTheDocument()
    })

    const select = screen.getByRole('combobox')
    expect(select).toHaveValue('0')

    // Check options include CPU only + adapters
    const options = screen.getAllByRole('option')
    expect(options).toHaveLength(3)
    expect(options[0]).toHaveTextContent('CPU only')
    expect(options[1]).toHaveTextContent('NVIDIA GeForce RTX 3080 (Vulkan)')
    expect(options[2]).toHaveTextContent('NVIDIA GeForce RTX 3080 (Dx12)')
  })

  it('shows CPU only selected when no active GPU', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_gpu_adapters') return [vulkanAdapter]
      if (cmd === 'get_active_gpu') return null
      return null
    })

    render(<GpuAdapterSelector />)

    await waitFor(() => {
      expect(screen.getByRole('combobox')).toBeInTheDocument()
    })

    expect(screen.getByRole('combobox')).toHaveValue('cpu')
  })

  it('handles adapter switch', async () => {
    const user = userEvent.setup()

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_gpu_adapters') return [vulkanAdapter]
      if (cmd === 'get_active_gpu') return null
      if (cmd === 'set_gpu_adapter') return 'NVIDIA GeForce RTX 3080'
      return null
    })

    render(<GpuAdapterSelector />)

    await waitFor(() => {
      expect(screen.getByRole('combobox')).toBeInTheDocument()
    })

    await act(async () => {
      await user.selectOptions(screen.getByRole('combobox'), '0')
    })

    expect(mockInvoke).toHaveBeenCalledWith('set_gpu_adapter', { adapterIndex: 0 })
  })

  it('handles switch to CPU only', async () => {
    const user = userEvent.setup()

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_gpu_adapters') return [vulkanAdapter]
      if (cmd === 'get_active_gpu') return vulkanAdapter
      if (cmd === 'set_gpu_adapter') return null
      return null
    })

    render(<GpuAdapterSelector />)

    await waitFor(() => {
      expect(screen.getByRole('combobox')).toBeInTheDocument()
    })

    await act(async () => {
      await user.selectOptions(screen.getByRole('combobox'), 'cpu')
    })

    expect(mockInvoke).toHaveBeenCalledWith('set_gpu_adapter', { adapterIndex: null })
  })

  it('renders empty adapter list gracefully', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_gpu_adapters') return []
      if (cmd === 'get_active_gpu') return null
      return null
    })

    render(<GpuAdapterSelector />)

    await waitFor(() => {
      expect(screen.getByRole('combobox')).toBeInTheDocument()
    })

    // Only CPU option
    const options = screen.getAllByRole('option')
    expect(options).toHaveLength(1)
    expect(options[0]).toHaveTextContent('CPU only')
  })

  it('has accessible label', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_gpu_adapters') return [vulkanAdapter]
      if (cmd === 'get_active_gpu') return vulkanAdapter
      return null
    })

    render(<GpuAdapterSelector />)

    await waitFor(() => {
      expect(screen.getByRole('combobox')).toBeInTheDocument()
    })

    expect(screen.getByLabelText('Select GPU adapter for frame processing')).toBeInTheDocument()
  })
})
