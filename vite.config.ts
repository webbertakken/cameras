import { resolve } from 'path'
import { playwright } from '@vitest/browser-playwright'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

const host = process.env.TAURI_DEV_HOST

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 5174,
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: 'unit',
          environment: 'jsdom',
          setupFiles: ['./src/test-setup.ts'],
          include: ['src/**/*.test.{ts,tsx}'],
          exclude: ['src/**/*.visual.test.{ts,tsx}'],
        },
      },
      {
        extends: true,
        test: {
          name: 'visual',
          include: ['src/**/*.visual.test.{ts,tsx}'],
          browser: {
            enabled: true,
            provider: playwright({
              contextOptions: { colorScheme: 'dark' },
            }),
            instances: [{ browser: 'chromium' }],
            orchestratorScripts: [
              {
                content: `localStorage.setItem('vueuse-color-scheme', 'dark')`,
                type: 'module',
              },
            ],
            screenshotFailures: false,
            expect: {
              toMatchScreenshot: {
                resolveScreenshotPath: ({
                  arg,
                  ext,
                  root,
                  testFileDirectory,
                  screenshotDirectory,
                  testFileName,
                  browserName,
                }) =>
                  resolve(
                    root,
                    testFileDirectory,
                    screenshotDirectory,
                    testFileName,
                    `${arg}-${browserName}${ext}`,
                  ),
                resolveDiffPath: ({
                  arg,
                  ext,
                  root,
                  attachmentsDir,
                  testFileDirectory,
                  testFileName,
                  browserName,
                }) =>
                  resolve(
                    root,
                    attachmentsDir,
                    testFileDirectory,
                    testFileName,
                    `${arg}-${browserName}${ext}`,
                  ),
              },
            },
          },
        },
      },
    ],
  },
}))
