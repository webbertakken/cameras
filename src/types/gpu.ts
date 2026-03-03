/** Information about a GPU adapter — matches Rust GpuAdapterInfo. */
export interface GpuAdapterInfo {
  index: number
  name: string
  backend: string
  deviceType: string
}
