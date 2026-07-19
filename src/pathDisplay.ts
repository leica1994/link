const WINDOWS_DEVICE_PATH_MARKER = '\\\\?'

export const formatPathForDisplay = (path: string) => {
  if (!path.startsWith(WINDOWS_DEVICE_PATH_MARKER) || path[3] !== '\\') {
    return path
  }

  const pathWithoutDevicePrefix = path.slice(4)
  if (pathWithoutDevicePrefix.toUpperCase().startsWith('UNC\\')) {
    return `\\\\${pathWithoutDevicePrefix.slice(4)}`
  }

  return pathWithoutDevicePrefix
}
