import { writable } from 'svelte/store'

/**
 * Overwrite-mode safety net (M-keynav §5/§6).
 *
 * CodeMirror 6 has no built-in overwrite/typeover mode, and we hard-block the
 * Insert key at the keymap level (see codemirror-setup.ts) so it can never be
 * toggled the usual way. This store is the *defensive* second line: if overwrite
 * mode is somehow activated through any path (an Insert-block edge case, a future
 * extension, or a browser quirk), something flips this true and the status bar
 * shows an "OVR" indicator so the user gets visible feedback that an unexpected
 * state occurred. Hidden (false) by default.
 */
export const overwriteMode = writable(false)

/** Force the overwrite-mode flag. The status bar mirrors it. */
export function setOverwriteMode(on: boolean): void {
  overwriteMode.set(on)
}

/**
 * Inspect a CodeMirror EditorView-like object for an active overwrite state and
 * mirror it into the store. CM6 exposes no public overwrite flag, so we probe a
 * couple of known shapes defensively (internal `inputState.overwrite`, or a DOM
 * dataset marker some integrations set). Safe to call with anything — it only
 * reads, never throws. Returns the detected state.
 */
export function syncOverwriteFromView(view: unknown): boolean {
  let on = false
  try {
    const v = view as { inputState?: { overwrite?: boolean }; contentDOM?: HTMLElement }
    if (v?.inputState?.overwrite === true) on = true
    if (v?.contentDOM?.dataset?.overwrite === 'true') on = true
  } catch {
    on = false
  }
  overwriteMode.set(on)
  return on
}
