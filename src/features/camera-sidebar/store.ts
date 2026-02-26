import { create } from 'zustand'
import type { CameraDevice } from '../../types/camera'

interface CameraStore {
  cameras: CameraDevice[]
  selectedId: string | null
  setCameras: (cameras: CameraDevice[]) => void
  selectCamera: (id: string) => void
  addCamera: (device: CameraDevice) => void
  removeCamera: (id: string) => void
  selectedCamera: () => CameraDevice | undefined
}

export const useCameraStore = create<CameraStore>((set, get) => ({
  cameras: [],
  selectedId: null,

  setCameras: (cameras) => set({ cameras }),

  selectCamera: (id) => set({ selectedId: id }),

  addCamera: (device) => set((state) => ({ cameras: [...state.cameras, device] })),

  removeCamera: (id) =>
    set((state) => {
      const idx = state.cameras.findIndex((c) => c.id === id)
      const remaining = state.cameras.filter((c) => c.id !== id)

      if (state.selectedId !== id) {
        return { cameras: remaining }
      }

      // Auto-select the next camera, or previous if last was removed
      const nextId =
        remaining.length === 0 ? null : remaining[Math.min(idx, remaining.length - 1)].id

      return { cameras: remaining, selectedId: nextId }
    }),

  selectedCamera: () => {
    const { cameras, selectedId } = get()
    return cameras.find((c) => c.id === selectedId)
  },
}))
