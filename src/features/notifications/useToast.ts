import { create } from 'zustand'

export type ToastType = 'success' | 'info'

export interface ToastItem {
  id: string
  message: string
  type: ToastType
}

interface ToastStore {
  toasts: ToastItem[]
  addToast: (message: string, type: ToastType) => void
  removeToast: (id: string) => void
}

let nextId = 0

export const useToastStore = create<ToastStore>((set, get) => ({
  toasts: [],

  addToast: (message, type) => {
    const id = String(++nextId)
    set((state) => ({ toasts: [...state.toasts, { id, message, type }] }))

    setTimeout(() => {
      // Guard against removal of already-dismissed toast
      if (get().toasts.some((t) => t.id === id)) {
        set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }))
      }
    }, 4000)
  },

  removeToast: (id) => {
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }))
  },
}))
