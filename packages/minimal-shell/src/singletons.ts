// Publish React globals BEFORE any remote extension loads (the federation contract).
// An extension's bare `import "react"` resolves to the shim → globalThis.__lbReact → THIS copy.
import React from "react";
import ReactDOM from "react-dom";
import * as ReactDOMClient from "react-dom/client";
import * as JSXRuntime from "react/jsx-runtime";

(globalThis).__lbReact = React as any;
(globalThis).__lbReactDom = ReactDOM as any;
(globalThis).__lbReactDomClient = ReactDOMClient as any;
(globalThis).__lbReactJsxRuntime = JSXRuntime as any;

export {};
