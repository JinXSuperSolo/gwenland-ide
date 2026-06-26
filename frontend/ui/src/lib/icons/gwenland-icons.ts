import file from 'material-icon-theme/icons/file.svg?raw'
import folder from 'material-icon-theme/icons/folder.svg?raw'
import folderOpen from 'material-icon-theme/icons/folder-open.svg?raw'
import node from 'material-icon-theme/icons/nodejs.svg?raw'
import npm from 'material-icon-theme/icons/npm.svg?raw'
import pnpm from 'material-icon-theme/icons/pnpm.svg?raw'
import rust from 'material-icon-theme/icons/rust.svg?raw'
import tauri from 'material-icon-theme/icons/tauri.svg?raw'
import docker from 'material-icon-theme/icons/docker.svg?raw'
import git from 'material-icon-theme/icons/git.svg?raw'
import lock from 'material-icon-theme/icons/lock.svg?raw'
import toml from 'material-icon-theme/icons/toml.svg?raw'
import json from 'material-icon-theme/icons/json.svg?raw'
import markdown from 'material-icon-theme/icons/markdown.svg?raw'
import mdx from 'material-icon-theme/icons/mdx.svg?raw'
import readme from 'material-icon-theme/icons/readme.svg?raw'
import license from 'material-icon-theme/icons/license.svg?raw'
import html from 'material-icon-theme/icons/html.svg?raw'
import css from 'material-icon-theme/icons/css.svg?raw'
import sass from 'material-icon-theme/icons/sass.svg?raw'
import less from 'material-icon-theme/icons/less.svg?raw'
import tailwind from 'material-icon-theme/icons/tailwindcss.svg?raw'
import javascript from 'material-icon-theme/icons/javascript.svg?raw'
import typescript from 'material-icon-theme/icons/typescript.svg?raw'
import typescriptDef from 'material-icon-theme/icons/typescript-def.svg?raw'
import react from 'material-icon-theme/icons/react.svg?raw'
import reactTs from 'material-icon-theme/icons/react_ts.svg?raw'
import svelte from 'material-icon-theme/icons/svelte.svg?raw'
import vue from 'material-icon-theme/icons/vue.svg?raw'
import python from 'material-icon-theme/icons/python.svg?raw'
import go from 'material-icon-theme/icons/go.svg?raw'
import java from 'material-icon-theme/icons/java.svg?raw'
import c from 'material-icon-theme/icons/c.svg?raw'
import cpp from 'material-icon-theme/icons/cpp.svg?raw'
import csharp from 'material-icon-theme/icons/csharp.svg?raw'
import php from 'material-icon-theme/icons/php.svg?raw'
import ruby from 'material-icon-theme/icons/ruby.svg?raw'
import lua from 'material-icon-theme/icons/lua.svg?raw'
import dart from 'material-icon-theme/icons/dart.svg?raw'
import yaml from 'material-icon-theme/icons/yaml.svg?raw'
import xml from 'material-icon-theme/icons/xml.svg?raw'
import graphql from 'material-icon-theme/icons/graphql.svg?raw'
import prisma from 'material-icon-theme/icons/prisma.svg?raw'
import database from 'material-icon-theme/icons/database.svg?raw'
import consoleIcon from 'material-icon-theme/icons/console.svg?raw'
import powershell from 'material-icon-theme/icons/powershell.svg?raw'
import image from 'material-icon-theme/icons/image.svg?raw'
import pdf from 'material-icon-theme/icons/pdf.svg?raw'
import word from 'material-icon-theme/icons/word.svg?raw'
import powerpoint from 'material-icon-theme/icons/powerpoint.svg?raw'
import audio from 'material-icon-theme/icons/audio.svg?raw'
import video from 'material-icon-theme/icons/video.svg?raw'
import font from 'material-icon-theme/icons/font.svg?raw'
import zip from 'material-icon-theme/icons/zip.svg?raw'
import vite from 'material-icon-theme/icons/vite.svg?raw'
import vitest from 'material-icon-theme/icons/vitest.svg?raw'
import eslint from 'material-icon-theme/icons/eslint.svg?raw'
import prettier from 'material-icon-theme/icons/prettier.svg?raw'
import editorconfig from 'material-icon-theme/icons/editorconfig.svg?raw'
import text from 'material-icon-theme/icons/document.svg?raw'

export const genericFileIcon = file

export const folderIcons = {
  closed: folder,
  open: folderOpen,
}

export const specialFileIcons: Record<string, string> = {
  'package.json': node,
  'package-lock.json': npm,
  'pnpm-lock.yaml': pnpm,
  'bun.lockb': lock,
  'cargo.toml': rust,
  'cargo.lock': lock,
  'tauri.conf.json': tauri,
  dockerfile: docker,
  '.dockerignore': docker,
  '.gitignore': git,
  '.gitattributes': git,
  '.env': lock,
  '.env.local': lock,
  '.env.production': lock,
  '.env.development': lock,
  'tsconfig.json': typescript,
  'jsconfig.json': javascript,
  'vite.config.ts': vite,
  'vite.config.js': vite,
  'vitest.config.ts': vitest,
  'eslint.config.js': eslint,
  'eslint.config.mjs': eslint,
  '.eslintrc': eslint,
  '.prettierrc': prettier,
  '.editorconfig': editorconfig,
  readme: readme,
  'readme.md': readme,
  license: license,
}

export const fileIcons: Record<string, string> = {
  ts: typescript,
  mts: typescript,
  cts: typescript,
  tsx: reactTs,
  dts: typescriptDef,
  js: javascript,
  mjs: javascript,
  cjs: javascript,
  jsx: react,
  rs: rust,
  svelte,
  vue,
  json,
  jsonc: json,
  toml,
  md: markdown,
  mdx,
  markdown,
  css,
  scss: sass,
  sass,
  less,
  html,
  htm: html,
  py: python,
  go,
  java,
  c,
  h: c,
  cpp,
  cc: cpp,
  cxx: cpp,
  hpp: cpp,
  cs: csharp,
  php,
  rb: ruby,
  lua,
  dart,
  yml: yaml,
  yaml,
  xml,
  graphql,
  gql: graphql,
  prisma,
  sql: database,
  sh: consoleIcon,
  bash: consoleIcon,
  zsh: consoleIcon,
  fish: consoleIcon,
  ps1: powershell,
  png: image,
  jpg: image,
  jpeg: image,
  gif: image,
  svg: image,
  webp: image,
  ico: image,
  bmp: image,
  pdf,
  doc: word,
  docx: word,
  ppt: powerpoint,
  pptx: powerpoint,
  mp3: audio,
  wav: audio,
  mp4: video,
  mov: video,
  woff: font,
  woff2: font,
  ttf: font,
  otf: font,
  zip,
  gz: zip,
  tar: zip,
  lock,
  txt: text,
}

export function fileIconSvg(name: string): string {
  const lower = name.toLowerCase()
  if (specialFileIcons[lower]) return specialFileIcons[lower]
  const ext = lower.includes('.') ? lower.slice(lower.lastIndexOf('.') + 1) : ''
  return fileIcons[ext] ?? genericFileIcon
}

export function folderIconSvg(open: boolean): string {
  return open ? folderIcons.open : folderIcons.closed
}

// Canonical API aliases used by the task spec and any future callers.
/** Combined filename + extension → SVG map (filename entries take precedence). */
export const FILE_ICONS: Record<string, string> = { ...specialFileIcons, ...fileIcons }

/** Returns the SVG string for a filename: special filename first, then extension, then default. */
export const getFileIcon = fileIconSvg
