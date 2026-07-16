export function componentSlug(component: string): string {
  return component.replaceAll(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()
}
