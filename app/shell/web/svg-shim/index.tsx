// Web preview shim for `react-native-svg`. The real package works on web via its own web build, but
// under this RN-Web + esbuild-optimize combo its Fabric internals import native-only RN modules
// (codegenNativeComponent, TurboModuleRegistry, …) that react-native-web doesn't expose, and the
// pre-bundler can't resolve the chain. So for the BROWSER PREVIEW ONLY we map `react-native-svg` to
// thin wrappers over the DOM's own <svg> elements — same element names/props our components use, so
// GaugeRing and any svg-based chart render identically in the preview. On device the real
// react-native-svg is used (this alias is web-only, see vite.config.web.mts). This is a preview
// rendering shim, not a fake backend — no node behavior is reimplemented.

import React from 'react';

type AnyProps = Record<string, unknown> & { children?: React.ReactNode };

const el =
  (tag: string) =>
  ({ children, ...props }: AnyProps) =>
    React.createElement(tag, props as never, children);

export const Svg = ({ children, width, height, ...props }: AnyProps) =>
  React.createElement(
    'svg',
    { xmlns: 'http://www.w3.org/2000/svg', width, height, ...props } as never,
    children,
  );

export const Circle = el('circle');
export const Rect = el('rect');
export const Path = el('path');
export const Line = el('line');
export const G = el('g');
export const Text = el('text');
export const Defs = el('defs');
export const LinearGradient = el('linearGradient');
export const RadialGradient = el('radialGradient');
export const Stop = el('stop');
export const Polygon = el('polygon');
export const Polyline = el('polyline');
export const Ellipse = el('ellipse');

export default Svg;
