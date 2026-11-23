// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// Zero-dependency WebGL2 network graph renderer for LOTUS Explorer.

(function () {
  "use strict";

  const LABEL_POOL_SIZE = 32;
  const ALPHA_EPS = 0.012;
  const MAX_REPULSION_SAMPLES = 64;

  const BG_VERT = `#version 300 es
precision highp float;
out vec2 vUv;
void main() {
  vec2 pos = vec2((gl_VertexID == 1) ? 3.0 : -1.0, (gl_VertexID == 2) ? 3.0 : -1.0);
  vUv = pos * 0.5 + 0.5;
  gl_Position = vec4(pos, 0.0, 1.0);
}`;

  const BG_FRAG = `#version 300 es
precision mediump float;
in vec2 vUv;
uniform float uDark;
out vec4 outColor;
void main() {
  float d = length(vUv - vec2(0.5)) * 1.4142135;
  float t = clamp(d * d, 0.0, 1.0);
  vec3 darkCenter = vec3(0.071, 0.110, 0.180);
  vec3 darkEdge   = vec3(0.039, 0.059, 0.098);
  vec3 lightCenter = vec3(1.000, 1.000, 1.000);
  vec3 lightEdge   = vec3(0.953, 0.969, 0.988);
  vec3 col = mix(mix(lightCenter, lightEdge, t), mix(darkCenter, darkEdge, t), uDark);
  outColor = vec4(col, 1.0);
}`;

  const EDGE_VERT = `#version 300 es
precision highp float;
layout(location = 0) in vec3 aPos;
layout(location = 1) in float aWeight;
layout(location = 2) in float aFocus;
layout(location = 3) in float aEndpoint;
uniform mat4 uViewProj;
uniform float uZFlat;
out float vWeight;
out float vFocus;
out float vEndpoint;
void main() {
  vec3 p = vec3(aPos.xy, aPos.z * (1.0 - uZFlat));
  gl_Position = uViewProj * vec4(p, 1.0);
  vWeight = aWeight;
  vFocus = aFocus;
  vEndpoint = aEndpoint;
}`;

  const EDGE_FRAG = `#version 300 es
precision mediump float;
in float vWeight;
in float vFocus;
in float vEndpoint;
uniform float uDark;
uniform float uFade;
uniform float uHasSel;
uniform float uPulsePhase;
out vec4 outColor;
void main() {
  float w = clamp(log2(1.0 + vWeight) / 4.0, 0.0, 1.0);
  vec3 darkBase = vec3(0.42, 0.56, 0.78);
  vec3 darkHot  = vec3(0.58, 0.78, 1.00);
  vec3 lightBase = vec3(0.32, 0.42, 0.56);
  vec3 lightHot  = vec3(0.05, 0.36, 0.68);
  float isHot = step(0.85, vFocus) * uHasSel;
  vec3 baseCol = mix(lightBase, darkBase, uDark);
  vec3 hotCol = mix(lightHot, darkHot, uDark);
  vec3 col = mix(baseCol, hotCol, isHot);
  float wave = 1.0;
  if (isHot > 0.5 && uPulsePhase >= 0.0) {
    wave = 0.80 + 0.20 * sin(vEndpoint * 6.2831853 - uPulsePhase);
  }
  float baseAlpha = mix(0.16, 0.22, uDark) + w * mix(0.28, 0.34, uDark);
  float alpha = baseAlpha * vFocus * wave * uFade;
  outColor = vec4(col * alpha, alpha);
}`;

  const NODE_VERT = `#version 300 es
precision highp float;
layout(location = 0) in vec3 aPos;
layout(location = 1) in float aRadius;
layout(location = 2) in float aKind;
layout(location = 3) in float aFocus;
uniform mat4 uViewProj;
uniform float uZFlat;
uniform float uPointScale;
uniform float uMaxPointSize;
uniform float uIs3d;
out float vKind;
out float vFocus;
out float vDepthFade;
void main() {
  vec3 p = vec3(aPos.xy, aPos.z * (1.0 - uZFlat));
  vec4 clip = uViewProj * vec4(p, 1.0);
  gl_Position = clip;
  float persp = (uIs3d > 0.5) ? clamp(320.0 / max(clip.w, 12.0), 0.45, 2.6) : 1.0;
  float focusScale = (aFocus > 1.5) ? 1.32 : ((aFocus > 1.1) ? 1.12 : 1.0);
  float sz = aRadius * uPointScale * persp * focusScale;
  gl_PointSize = clamp(sz, 3.0, uMaxPointSize);
  vKind = aKind;
  vFocus = aFocus;
  vDepthFade = (uIs3d > 0.5) ? clamp(1.15 - (clip.z / max(clip.w, 1.0)) * 0.25, 0.65, 1.0) : 1.0;
}`;

  const NODE_FRAG = `#version 300 es
precision mediump float;
in float vKind;
in float vFocus;
in float vDepthFade;
uniform float uDark;
uniform float uFade;
out vec4 outColor;

float sdNode(vec2 p, float kind, float r) {
  if (kind < 0.5) {
    return length(p) - r;
  } else if (kind < 1.5) {
    vec2 q = abs(p);
    return (q.x + q.y) * 0.70710678 - r * 1.12;
  } else {
    vec2 b = vec2(r * 0.82);
    vec2 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r * 0.18;
  }
}

void main() {
  vec2 p = (gl_PointCoord - vec2(0.5)) * 2.0;
  float shapeR = 0.54;
  float d = sdNode(p, vKind, shapeR);
  float aa = max(fwidth(d), 0.045);

  vec3 darkFill = (vKind < 0.5)
    ? vec3(0.95, 0.44, 0.40)
    : ((vKind < 1.5) ? vec3(0.30, 0.78, 0.55) : vec3(0.45, 0.73, 0.98));
  vec3 lightFill = (vKind < 0.5)
    ? vec3(0.60, 0.05, 0.05)
    : ((vKind < 1.5) ? vec3(0.14, 0.50, 0.31) : vec3(0.03, 0.40, 0.60));
  vec3 lightStroke = (vKind < 0.5)
    ? vec3(0.38, 0.00, 0.00)
    : ((vKind < 1.5) ? vec3(0.07, 0.29, 0.18) : vec3(0.00, 0.23, 0.38));

  vec3 fill = mix(lightFill, darkFill, uDark);
  float bodyMask = 1.0 - smoothstep(-aa, aa, d);
  float edgeMask = 1.0 - smoothstep(0.75, 1.0, length(p));
  float focusAlpha = clamp(vFocus, 0.14, 1.0) * uFade * vDepthFade;

  if (uDark > 0.5) {
    float halo = exp(-max(d, 0.0) * 4.8) * (vFocus > 1.05 ? 0.58 : 0.28) * edgeMask;
    float coreHighlight = (1.0 - smoothstep(-aa, aa, d + 0.20)) * 0.22;
    vec3 ringCol = vec3(0.95, 0.97, 1.0);
    float ring = (vFocus > 1.5)
      ? (1.0 - smoothstep(0.04, 0.04 + aa, abs(d - 0.14)))
      : 0.0;
    vec3 rgb = fill * (bodyMask + halo * 0.85) + vec3(coreHighlight) + ringCol * ring;
    float alpha = clamp(max(bodyMask, max(halo * 0.65, ring)) * focusAlpha, 0.0, 1.0);
    outColor = vec4(rgb * focusAlpha, alpha);
  } else {
    float border = (1.0 - smoothstep(0.0, aa * 1.4, abs(d))) * bodyMask;
    float aura = exp(-max(d, 0.0) * 6.0) * (vFocus > 1.05 ? 0.24 : 0.06) * edgeMask;
    float ring = (vFocus > 1.5)
      ? (1.0 - smoothstep(0.05, 0.05 + aa, abs(d - 0.14)))
      : 0.0;
    vec3 ringCol = vec3(0.043, 0.361, 0.671);
    vec3 bodyCol = mix(fill, lightStroke, border * 0.65);
    vec3 rgb = bodyCol * bodyMask + fill * aura * 0.5 + ringCol * ring;
    float alpha = clamp(max(bodyMask, max(aura * 0.5, ring)) * focusAlpha, 0.0, 1.0);
    outColor = vec4(rgb * focusAlpha, alpha);
  }
}`;

  function compileShader(gl, type, src) {
    const shader = gl.createShader(type);
    if (!shader) return null;
    gl.shaderSource(shader, src);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      gl.deleteShader(shader);
      return null;
    }
    return shader;
  }

  function createProgram(gl, vertSrc, fragSrc) {
    const vs = compileShader(gl, gl.VERTEX_SHADER, vertSrc);
    const fs = compileShader(gl, gl.FRAGMENT_SHADER, fragSrc);
    if (!vs || !fs) return null;
    const prog = gl.createProgram();
    if (!prog) return null;
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      gl.deleteProgram(prog);
      return null;
    }
    return prog;
  }

  function mat4Multiply(a, b) {
    const out = new Float32Array(16);
    for (let col = 0; col < 4; col += 1) {
      const b0 = b[col * 4];
      const b1 = b[col * 4 + 1];
      const b2 = b[col * 4 + 2];
      const b3 = b[col * 4 + 3];
      out[col * 4] = a[0] * b0 + a[4] * b1 + a[8] * b2 + a[12] * b3;
      out[col * 4 + 1] = a[1] * b0 + a[5] * b1 + a[9] * b2 + a[13] * b3;
      out[col * 4 + 2] = a[2] * b0 + a[6] * b1 + a[10] * b2 + a[14] * b3;
      out[col * 4 + 3] = a[3] * b0 + a[7] * b1 + a[11] * b2 + a[15] * b3;
    }
    return out;
  }

  function mat4Perspective(fovRad, aspect, near, far) {
    const f = 1.0 / Math.tan(fovRad * 0.5);
    const nf = 1.0 / (near - far);
    const out = new Float32Array(16);
    out[0] = f / aspect;
    out[5] = f;
    out[10] = (far + near) * nf;
    out[11] = -1;
    out[14] = 2 * far * near * nf;
    return out;
  }

  function mat4Ortho(left, right, bottom, top, near, far) {
    const lr = 1.0 / (left - right);
    const bt = 1.0 / (bottom - top);
    const nf = 1.0 / (near - far);
    const out = new Float32Array(16);
    out[0] = -2 * lr;
    out[5] = -2 * bt;
    out[10] = 2 * nf;
    out[12] = (left + right) * lr;
    out[13] = (top + bottom) * bt;
    out[14] = (far + near) * nf;
    out[15] = 1;
    return out;
  }

  function mat4LookAt(eye, center, up) {
    let zx = eye[0] - center[0];
    let zy = eye[1] - center[1];
    let zz = eye[2] - center[2];
    let len = Math.hypot(zx, zy, zz) || 1;
    zx /= len; zy /= len; zz /= len;

    let xx = up[1] * zz - up[2] * zy;
    let xy = up[2] * zx - up[0] * zz;
    let xz = up[0] * zy - up[1] * zx;
    len = Math.hypot(xx, xy, xz) || 1;
    xx /= len; xy /= len; xz /= len;

    const yx = zy * xz - zz * xy;
    const yy = zz * xx - zx * xz;
    const yz = zx * xy - zy * xx;

    const out = new Float32Array(16);
    out[0] = xx; out[4] = xy; out[8] = xz; out[12] = -(xx * eye[0] + xy * eye[1] + xz * eye[2]);
    out[1] = yx; out[5] = yy; out[9] = yz; out[13] = -(yx * eye[0] + yy * eye[1] + yz * eye[2]);
    out[2] = zx; out[6] = zy; out[10] = zz; out[14] = -(zx * eye[0] + zy * eye[1] + zz * eye[2]);
    out[3] = 0; out[7] = 0; out[11] = 0; out[15] = 1;
    return out;
  }

  function emit(detail) {
    window.dispatchEvent(new CustomEvent("lotus-graph-event", { detail }));
  }

  function prefersReducedMotion() {
    return typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  const state = {
    canvas: null,
    labelsHost: null,
    labelPool: [],
    gl: null,
    opts: { nativeDownload: false },
    programs: null,
    buffers: null,
    vaos: null,
    resizeObserver: null,
    abortController: null,
    rafId: 0,
    resizeRaf: 0,
    destroyed: false,
    contextLost: false,
    unavailable: false,
    dpr: 1,
    width: 1,
    height: 1,
    maxPointSize: 64,
    dark: false,
    mode: "3d",
    paused: false,
    loading: false,
    reducedMotion: false,
    fade: 1.0,
    fadeGoal: 1.0,
    zFlat: 0.0,
    zFlatGoal: 0.0,
    pulseUntil: 0,
    pulsePhase: -1.0,

    // Graph data
    nNodes: 0,
    nEdges: 0,
    kinds: new Float32Array(0),
    radii: new Float32Array(0),
    degrees: new Uint32Array(0),
    labels: [],
    topHubs: [],
    edges: new Uint32Array(0),
    edgeWeights: new Float32Array(0),
    adj: [],

    // Layout & buffers
    pos: new Float32Array(0),
    pos3d: new Float32Array(0),
    vel: new Float32Array(0),
    fx: new Float32Array(0),
    fy: new Float32Array(0),
    fz: new Float32Array(0),
    focusArr: new Float32Array(0),
    drawOrder: new Uint32Array(0),
    edgePos: new Float32Array(0),
    edgeMeta: new Float32Array(0),
    alpha: 0,
    starGraph: false,
    positionsDirty: false,
    focusDirty: false,

    // Picking & camera
    screenX: new Float32Array(0),
    screenY: new Float32Array(0),
    screenR: new Float32Array(0),
    screenVis: new Uint8Array(0),
    selectedIdx: null,
    hoverIdx: null,
    viewProj: new Float32Array(16),
    cam: {
      yaw: 0.45,
      pitch: 0.35,
      dist: 420,
      tx: 0,
      ty: 0,
      tz: 0,
      yawGoal: 0.45,
      pitchGoal: 0.35,
      distGoal: 420,
      txGoal: 0,
      tyGoal: 0,
      tzGoal: 0,
    },
    pointer: {
      down: false,
      panMode: false,
      moved: false,
      startX: 0,
      startY: 0,
      lastX: 0,
      lastY: 0,
    },
  };

  function initGLResources() {
    const gl = state.gl;
    if (!gl) return false;
    const bgProg = createProgram(gl, BG_VERT, BG_FRAG);
    const edgeProg = createProgram(gl, EDGE_VERT, EDGE_FRAG);
    const nodeProg = createProgram(gl, NODE_VERT, NODE_FRAG);
    if (!bgProg || !edgeProg || !nodeProg) return false;

    const ptRange = gl.getParameter(gl.ALIASED_POINT_SIZE_RANGE);
    state.maxPointSize = ptRange && ptRange[1] ? Math.min(ptRange[1], 128) : 64;

    state.programs = {
      bg: {
        prog: bgProg,
        uDark: gl.getUniformLocation(bgProg, "uDark"),
      },
      edge: {
        prog: edgeProg,
        uViewProj: gl.getUniformLocation(edgeProg, "uViewProj"),
        uZFlat: gl.getUniformLocation(edgeProg, "uZFlat"),
        uDark: gl.getUniformLocation(edgeProg, "uDark"),
        uFade: gl.getUniformLocation(edgeProg, "uFade"),
        uHasSel: gl.getUniformLocation(edgeProg, "uHasSel"),
        uPulsePhase: gl.getUniformLocation(edgeProg, "uPulsePhase"),
      },
      node: {
        prog: nodeProg,
        uViewProj: gl.getUniformLocation(nodeProg, "uViewProj"),
        uZFlat: gl.getUniformLocation(nodeProg, "uZFlat"),
        uPointScale: gl.getUniformLocation(nodeProg, "uPointScale"),
        uMaxPointSize: gl.getUniformLocation(nodeProg, "uMaxPointSize"),
        uIs3d: gl.getUniformLocation(nodeProg, "uIs3d"),
        uDark: gl.getUniformLocation(nodeProg, "uDark"),
        uFade: gl.getUniformLocation(nodeProg, "uFade"),
      },
    };

    state.vaos = {
      bg: gl.createVertexArray(),
      edge: gl.createVertexArray(),
      node: gl.createVertexArray(),
    };
    state.buffers = {
      edgePos: gl.createBuffer(),
      edgeMeta: gl.createBuffer(),
      nodePos: gl.createBuffer(),
      nodeRadius: gl.createBuffer(),
      nodeKind: gl.createBuffer(),
      nodeFocus: gl.createBuffer(),
      nodeOrder: gl.createBuffer(),
    };

    gl.bindVertexArray(state.vaos.edge);
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.edgePos);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.edgeMeta);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 1, gl.FLOAT, false, 12, 0);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 1, gl.FLOAT, false, 12, 4);
    gl.enableVertexAttribArray(3);
    gl.vertexAttribPointer(3, 1, gl.FLOAT, false, 12, 8);

    gl.bindVertexArray(state.vaos.node);
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.nodePos);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.nodeRadius);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 1, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.nodeKind);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 1, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.nodeFocus);
    gl.enableVertexAttribArray(3);
    gl.vertexAttribPointer(3, 1, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, state.buffers.nodeOrder);

    gl.bindVertexArray(null);
    return true;
  }

  function resizeCanvas() {
    if (!state.canvas) return;
    const host = state.canvas.parentElement || state.canvas;
    let w = Math.floor(host.clientWidth || 0);
    let h = Math.floor(host.clientHeight || 0);
    if (w < 1 || h < 1) {
      const rect = host.getBoundingClientRect();
      w = Math.max(1, Math.floor(rect.width));
      h = Math.max(1, Math.floor(rect.height));
    }
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const targetW = Math.floor(w * dpr);
    const targetH = Math.floor(h * dpr);
    if (
      w !== state.width ||
      h !== state.height ||
      dpr !== state.dpr ||
      state.canvas.width !== targetW ||
      state.canvas.height !== targetH
    ) {
      state.width = w;
      state.height = h;
      state.dpr = dpr;
      state.canvas.style.width = `${w}px`;
      state.canvas.style.height = `${h}px`;
      state.canvas.width = targetW;
      state.canvas.height = targetH;
      scheduleFrame();
    }
  }

  function scheduleResize() {
    if (state.resizeRaf !== 0 || state.destroyed) return;
    state.resizeRaf = window.requestAnimationFrame(() => {
      state.resizeRaf = 0;
      resizeCanvas();
    });
  }

  function ensureLabelPool() {
    if (!state.labelsHost) return;
    if (state.labelPool.length === LABEL_POOL_SIZE) return;
    state.labelsHost.textContent = "";
    state.labelPool = [];
    for (let i = 0; i < LABEL_POOL_SIZE; i += 1) {
      const span = document.createElement("span");
      span.className = "lotus-graph-label";
      span.setAttribute("aria-hidden", "true");
      span.style.display = "none";
      state.labelsHost.appendChild(span);
      state.labelPool.push(span);
    }
  }

  function updateFocusBuffers() {
    const n = state.nNodes;
    const sel = state.selectedIdx;
    const hov = state.hoverIdx;
    const neighborSet = sel !== null && state.adj[sel] ? state.adj[sel] : null;
    const bgFocus = sel !== null ? 0.18 : 1.0;

    for (let i = 0; i < n; i += 1) {
      let f = bgFocus;
      if (i === sel) {
        f = 2.0;
      } else if (neighborSet && neighborSet.has(i)) {
        f = 1.35;
      }
      if (i === hov && f < 1.8) {
        f = 1.65;
      }
      state.focusArr[i] = f;
    }

    const bg = [];
    const mid = [];
    const top = [];
    for (let i = 0; i < n; i += 1) {
      if (i === sel || i === hov) {
        top.push(i);
      } else if (neighborSet && neighborSet.has(i)) {
        mid.push(i);
      } else {
        bg.push(i);
      }
    }
    let cursor = 0;
    for (let i = 0; i < bg.length; i += 1) state.drawOrder[cursor++] = bg[i];
    for (let i = 0; i < mid.length; i += 1) state.drawOrder[cursor++] = mid[i];
    for (let i = 0; i < top.length; i += 1) state.drawOrder[cursor++] = top[i];

    const e = state.nEdges;
    for (let k = 0; k < e; k += 1) {
      const a = state.edges[k * 2];
      const b = state.edges[k * 2 + 1];
      const hot = sel === null || a === sel || b === sel;
      const focus = sel === null ? 1.0 : (hot ? 1.0 : 0.10);
      const w = state.edgeWeights[k];
      const o0 = k * 6;
      state.edgeMeta[o0] = w;
      state.edgeMeta[o0 + 1] = focus;
      state.edgeMeta[o0 + 2] = a === sel ? 0.0 : 1.0;
      state.edgeMeta[o0 + 3] = w;
      state.edgeMeta[o0 + 4] = focus;
      state.edgeMeta[o0 + 5] = b === sel ? 0.0 : 1.0;
    }

    state.focusDirty = true;
  }

  function uploadAllStaticBuffers() {
    const gl = state.gl;
    if (!gl || !state.buffers) return;
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.nodeRadius);
    gl.bufferData(gl.ARRAY_BUFFER, state.radii, gl.STATIC_DRAW);
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.nodeKind);
    gl.bufferData(gl.ARRAY_BUFFER, state.kinds, gl.STATIC_DRAW);
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.nodePos);
    gl.bufferData(gl.ARRAY_BUFFER, state.pos, gl.DYNAMIC_DRAW);
    gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.edgePos);
    gl.bufferData(gl.ARRAY_BUFFER, state.edgePos, gl.DYNAMIC_DRAW);
    state.focusDirty = true;
    state.positionsDirty = true;
  }

  function layoutStarGraph() {
    const n = state.nNodes;
    let hub = 0;
    for (let i = 1; i < n; i += 1) {
      if (state.degrees[i] > state.degrees[hub]) hub = i;
    }
    const others = [];
    for (let i = 0; i < n; i += 1) {
      if (i !== hub) {
        let w = 1;
        for (let k = 0; k < state.nEdges; k += 1) {
          const a = state.edges[k * 2];
          const b = state.edges[k * 2 + 1];
          if ((a === hub && b === i) || (b === hub && a === i)) {
            w = state.edgeWeights[k];
            break;
          }
        }
        others.push({ idx: i, weight: w });
      }
    }
    others.sort((a, b) => b.weight - a.weight || a.idx - b.idx);

    state.pos[hub * 3] = 0;
    state.pos[hub * 3 + 1] = 0;
    state.pos[hub * 3 + 2] = 0;
    state.pos3d[hub * 3] = 0;
    state.pos3d[hub * 3 + 1] = 0;
    state.pos3d[hub * 3 + 2] = 0;

    const perShell = 28;
    const baseR = 92;
    const shellStep = 58;
    for (let k = 0; k < others.length; k += 1) {
      const idx = others[k].idx;
      const shell = Math.floor(k / perShell);
      const countInShell = Math.min(perShell, others.length - shell * perShell);
      const posInShell = k % perShell;
      const angle = (posInShell / Math.max(1, countInShell)) * Math.PI * 2 + shell * 0.45;
      const r = baseR + shell * shellStep;
      const x = Math.cos(angle) * r;
      const y = Math.sin(angle) * r;
      const z = ((k % 5) - 2) * 14;
      state.pos[idx * 3] = x;
      state.pos[idx * 3 + 1] = y;
      state.pos[idx * 3 + 2] = z;
      state.pos3d[idx * 3] = x;
      state.pos3d[idx * 3 + 1] = y;
      state.pos3d[idx * 3 + 2] = z;
    }
    state.alpha = 0;
    syncEdgePositions();
    state.positionsDirty = true;
  }

  function syncEdgePositions() {
    const e = state.nEdges;
    for (let k = 0; k < e; k += 1) {
      const s = state.edges[k * 2] * 3;
      const t = state.edges[k * 2 + 1] * 3;
      const o = k * 6;
      state.edgePos[o] = state.pos3d[s];
      state.edgePos[o + 1] = state.pos3d[s + 1];
      state.edgePos[o + 2] = state.pos3d[s + 2];
      state.edgePos[o + 3] = state.pos3d[t];
      state.edgePos[o + 4] = state.pos3d[t + 1];
      state.edgePos[o + 5] = state.pos3d[t + 2];
      state.pos[s] = state.pos3d[s];
      state.pos[s + 1] = state.pos3d[s + 1];
      state.pos[s + 2] = state.pos3d[s + 2];
      state.pos[t] = state.pos3d[t];
      state.pos[t + 1] = state.pos3d[t + 1];
      state.pos[t + 2] = state.pos3d[t + 2];
    }
    if (e === 0 && state.nNodes > 0) {
      state.pos.set(state.pos3d);
    }
  }

  function stepForceLayout() {
    const n = state.nNodes;
    if (n <= 1 || state.alpha <= ALPHA_EPS) return;
    const fx = state.fx;
    const fy = state.fy;
    const fz = state.fz;
    fx.fill(0);
    fy.fill(0);
    fz.fill(0);

    const pos = state.pos3d;
    const alpha = state.alpha;
    const samples = Math.min(n - 1, MAX_REPULSION_SAMPLES);
    const stride = n - 1 <= MAX_REPULSION_SAMPLES ? 1 : Math.max(1, Math.floor((n - 1) / MAX_REPULSION_SAMPLES));
    const repScale = (n > MAX_REPULSION_SAMPLES ? (n - 1) / samples : 1) * 520.0 * alpha;

    for (let i = 0; i < n; i += 1) {
      const ix = pos[i * 3];
      const iy = pos[i * 3 + 1];
      const iz = pos[i * 3 + 2];
      let accX = 0;
      let accY = 0;
      let accZ = 0;
      for (let step = 1; step <= samples; step += 1) {
        const j = (i + step * stride) % n;
        if (j === i) continue;
        const dx = ix - pos[j * 3];
        const dy = iy - pos[j * 3 + 1];
        const dz = iz - pos[j * 3 + 2];
        const dist2 = dx * dx + dy * dy + dz * dz + 36.0;
        const inv = repScale / (dist2 * Math.sqrt(dist2));
        accX += dx * inv;
        accY += dy * inv;
        accZ += dz * inv;
      }
      fx[i] += accX;
      fy[i] += accY;
      fz[i] += accZ;
    }

    const e = state.nEdges;
    const restBase = 58.0;
    for (let k = 0; k < e; k += 1) {
      const a = state.edges[k * 2];
      const b = state.edges[k * 2 + 1];
      const dx = pos[b * 3] - pos[a * 3];
      const dy = pos[b * 3 + 1] - pos[a * 3 + 1];
      const dz = pos[b * 3 + 2] - pos[a * 3 + 2];
      const dist = Math.hypot(dx, dy, dz) || 1.0;
      const w = Math.min(4.0, 1.0 + Math.log2(1.0 + state.edgeWeights[k]) * 0.4);
      const rest = restBase / Math.sqrt(w);
      const force = ((dist - rest) / dist) * 0.045 * w * alpha;
      const sx = dx * force;
      const sy = dy * force;
      const sz = dz * force;
      fx[a] += sx; fy[a] += sy; fz[a] += sz;
      fx[b] -= sx; fy[b] -= sy; fz[b] -= sz;
    }

    for (let i = 0; i < n; i += 1) {
      const grav = (0.018 + Math.min(0.06, state.degrees[i] * 0.002)) * alpha;
      fx[i] -= pos[i * 3] * grav;
      fy[i] -= pos[i * 3 + 1] * grav;
      fz[i] -= pos[i * 3 + 2] * grav;

      const i3 = i * 3;
      state.vel[i3] = (state.vel[i3] + fx[i]) * 0.80;
      state.vel[i3 + 1] = (state.vel[i3 + 1] + fy[i]) * 0.80;
      state.vel[i3 + 2] = (state.vel[i3 + 2] + fz[i]) * 0.80;

      const maxStep = 14.0;
      const vLen = Math.hypot(state.vel[i3], state.vel[i3 + 1], state.vel[i3 + 2]);
      if (vLen > maxStep) {
        const s = maxStep / vLen;
        state.vel[i3] *= s;
        state.vel[i3 + 1] *= s;
        state.vel[i3 + 2] *= s;
      }

      pos[i3] += state.vel[i3];
      pos[i3 + 1] += state.vel[i3 + 1];
      pos[i3 + 2] += state.vel[i3 + 2];
    }

    syncEdgePositions();
    state.positionsDirty = true;
    state.alpha *= 0.965;
    if (state.alpha <= ALPHA_EPS) {
      state.alpha = 0;
      emit({ type: "settled" });
    }
  }

  function computeViewProj() {
    const aspect = Math.max(0.1, state.width / Math.max(1, state.height));
    const cam = state.cam;
    if (state.mode === "2d") {
      const halfH = Math.max(40, cam.dist * 0.45);
      const halfW = halfH * aspect;
      const proj = mat4Ortho(-halfW, halfW, -halfH, halfH, -2000, 2000);
      const view = mat4LookAt([cam.tx, cam.ty, 500], [cam.tx, cam.ty, 0], [0, 1, 0]);
      state.viewProj = mat4Multiply(proj, view);
      return;
    }
    const cosP = Math.cos(cam.pitch);
    const eye = [
      cam.tx + cam.dist * cosP * Math.sin(cam.yaw),
      cam.ty + cam.dist * Math.sin(cam.pitch),
      cam.tz + cam.dist * cosP * Math.cos(cam.yaw),
    ];
    const proj = mat4Perspective(0.78539816, aspect, 2.0, Math.max(4000, cam.dist * 8));
    const view = mat4LookAt(eye, [cam.tx, cam.ty, cam.tz], [0, 1, 0]);
    state.viewProj = mat4Multiply(proj, view);
  }

  function projectToScreen() {
    const n = state.nNodes;
    const m = state.viewProj;
    const w = state.width;
    const h = state.height;
    const zScale = 1.0 - state.zFlat;
    const is3d = state.mode === "3d";
    const ptScale = Math.min(1.8, Math.max(0.85, Math.min(w, h) / 560));

    for (let i = 0; i < n; i += 1) {
      const x = state.pos3d[i * 3];
      const y = state.pos3d[i * 3 + 1];
      const z = state.pos3d[i * 3 + 2] * zScale;
      const cx = m[0] * x + m[4] * y + m[8] * z + m[12];
      const cy = m[1] * x + m[5] * y + m[9] * z + m[13];
      const cw = m[3] * x + m[7] * y + m[11] * z + m[15];
      if (cw <= 0.001) {
        state.screenVis[i] = 0;
        continue;
      }
      const nx = cx / cw;
      const ny = cy / cw;
      if (nx < -1.2 || nx > 1.2 || ny < -1.2 || ny > 1.2) {
        state.screenVis[i] = 0;
        continue;
      }
      state.screenX[i] = (nx * 0.5 + 0.5) * w;
      state.screenY[i] = (0.5 - ny * 0.5) * h;
      const persp = is3d ? Math.min(2.6, Math.max(0.45, 320.0 / Math.max(cw, 12.0))) : 1.0;
      state.screenR[i] = 0.5 * state.radii[i] * ptScale * 1.4 * persp;
      state.screenVis[i] = 1;
    }
  }

  function updateDomLabels() {
    if (!state.labelsHost || state.nNodes === 0 || state.fade < 0.35) {
      for (let i = 0; i < state.labelPool.length; i += 1) {
        state.labelPool[i].style.display = "none";
      }
      return;
    }
    ensureLabelPool();

    const candidates = [];
    const seen = new Set();
    const pushCand = (idx, priority) => {
      if (idx === null || idx === undefined || idx < 0 || idx >= state.nNodes) return;
      if (!state.screenVis[idx] || seen.has(idx)) return;
      seen.add(idx);
      candidates.push({ idx, priority });
    };

    pushCand(state.hoverIdx, 3);
    pushCand(state.selectedIdx, 3);
    for (let i = 0; i < state.topHubs.length; i += 1) {
      pushCand(state.topHubs[i], 2);
    }
    if (state.selectedIdx !== null && state.adj[state.selectedIdx]) {
      state.adj[state.selectedIdx].forEach((nbr) => pushCand(nbr, 1));
    }

    const placed = [];
    let poolIdx = 0;
    const maxW = state.width - 12;
    const maxH = state.height - 12;

    for (let c = 0; c < candidates.length && poolIdx < state.labelPool.length; c += 1) {
      const { idx, priority } = candidates[c];
      const text = state.labels[idx] || "";
      if (!text) continue;
      const sx = state.screenX[idx];
      const sy = state.screenY[idx] - Math.max(8, state.screenR[idx] * 0.55) - 6;
      const estW = Math.min(180, text.length * 6.8 + 14);
      const estH = 18;
      const left = sx - estW * 0.5;
      const top = sy - estH;
      if (left < 8 || top < 8 || left + estW > maxW || top + estH > maxH) continue;

      let overlaps = false;
      for (let p = 0; p < placed.length; p += 1) {
        const box = placed[p];
        if (left < box.r + 4 && left + estW + 4 > box.l && top < box.b + 3 && top + estH + 3 > box.t) {
          overlaps = true;
          break;
        }
      }
      if (overlaps) continue;

      placed.push({ l: left, t: top, r: left + estW, b: top + estH });
      const span = state.labelPool[poolIdx++];
      span.textContent = text.length > 28 ? `${text.slice(0, 27)}…` : text;
      span.style.display = "block";
      span.style.transform = `translate3d(${Math.round(sx)}px, ${Math.round(sy)}px, 0) translate(-50%, -100%)`;
      span.classList.toggle("is-taxon", state.kinds[idx] === 1);
      span.classList.toggle("is-active", priority === 3);
    }

    for (let i = poolIdx; i < state.labelPool.length; i += 1) {
      state.labelPool[i].style.display = "none";
    }
  }

  function renderFrame() {
    const gl = state.gl;
    if (!gl || !state.programs || !state.vaos || state.contextLost) return;
    computeViewProj();

    gl.viewport(0, 0, state.canvas.width, state.canvas.height);
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.BLEND);

    gl.useProgram(state.programs.bg.prog);
    gl.uniform1f(state.programs.bg.uDark, state.dark ? 1.0 : 0.0);
    gl.bindVertexArray(state.vaos.bg);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    if (state.nNodes === 0) {
      gl.bindVertexArray(null);
      updateDomLabels();
      return;
    }

    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

    if (state.positionsDirty) {
      gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.nodePos);
      gl.bufferData(gl.ARRAY_BUFFER, state.pos3d, gl.DYNAMIC_DRAW);
      gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.edgePos);
      gl.bufferData(gl.ARRAY_BUFFER, state.edgePos, gl.DYNAMIC_DRAW);
      state.positionsDirty = false;
    }
    if (state.focusDirty) {
      gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.nodeFocus);
      gl.bufferData(gl.ARRAY_BUFFER, state.focusArr, gl.DYNAMIC_DRAW);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, state.buffers.nodeOrder);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, state.drawOrder, gl.DYNAMIC_DRAW);
      gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.edgeMeta);
      gl.bufferData(gl.ARRAY_BUFFER, state.edgeMeta, gl.DYNAMIC_DRAW);
      state.focusDirty = false;
    }

    if (state.nEdges > 0) {
      const pEdge = state.programs.edge;
      gl.useProgram(pEdge.prog);
      gl.uniformMatrix4fv(pEdge.uViewProj, false, state.viewProj);
      gl.uniform1f(pEdge.uZFlat, state.zFlat);
      gl.uniform1f(pEdge.uDark, state.dark ? 1.0 : 0.0);
      gl.uniform1f(pEdge.uFade, state.fade);
      gl.uniform1f(pEdge.uHasSel, state.selectedIdx !== null ? 1.0 : 0.0);
      gl.uniform1f(pEdge.uPulsePhase, state.pulsePhase);
      gl.bindVertexArray(state.vaos.edge);
      gl.drawArrays(gl.LINES, 0, state.nEdges * 2);
    }

    const pNode = state.programs.node;
    gl.useProgram(pNode.prog);
    gl.uniformMatrix4fv(pNode.uViewProj, false, state.viewProj);
    gl.uniform1f(pNode.uZFlat, state.zFlat);
    const pointScale = Math.min(1.8, Math.max(0.85, Math.min(state.width, state.height) / 560)) * state.dpr * 1.4;
    gl.uniform1f(pNode.uPointScale, pointScale);
    gl.uniform1f(pNode.uMaxPointSize, state.maxPointSize);
    gl.uniform1f(pNode.uIs3d, state.mode === "3d" ? 1.0 : 0.0);
    gl.uniform1f(pNode.uDark, state.dark ? 1.0 : 0.0);
    gl.uniform1f(pNode.uFade, state.fade);
    gl.bindVertexArray(state.vaos.node);
    gl.drawElements(gl.POINTS, state.nNodes, gl.UNSIGNED_INT, 0);
    gl.bindVertexArray(null);

    projectToScreen();
    updateDomLabels();
  }

  function tick(now) {
    state.rafId = 0;
    if (state.destroyed || state.contextLost) return;

    let needMore = false;
    if (!state.paused && !state.reducedMotion && state.alpha > ALPHA_EPS && !state.starGraph) {
      stepForceLayout();
      if (state.alpha > ALPHA_EPS) needMore = true;
    }

    const cam = state.cam;
    const damp = state.reducedMotion ? 1.0 : 0.18;
    const camErr =
      Math.abs(cam.yawGoal - cam.yaw) +
      Math.abs(cam.pitchGoal - cam.pitch) +
      Math.abs(cam.distGoal - cam.dist) * 0.005 +
      Math.hypot(cam.txGoal - cam.tx, cam.tyGoal - cam.ty, cam.tzGoal - cam.tz) * 0.01;
    if (camErr > 0.0008) {
      cam.yaw += (cam.yawGoal - cam.yaw) * damp;
      cam.pitch += (cam.pitchGoal - cam.pitch) * damp;
      cam.dist += (cam.distGoal - cam.dist) * damp;
      cam.tx += (cam.txGoal - cam.tx) * damp;
      cam.ty += (cam.tyGoal - cam.ty) * damp;
      cam.tz += (cam.tzGoal - cam.tz) * damp;
      if (!state.reducedMotion) needMore = true;
    } else {
      cam.yaw = cam.yawGoal;
      cam.pitch = cam.pitchGoal;
      cam.dist = cam.distGoal;
      cam.tx = cam.txGoal;
      cam.ty = cam.tyGoal;
      cam.tz = cam.tzGoal;
    }

    const interp = state.reducedMotion ? 1.0 : 0.18;
    if (Math.abs(state.fadeGoal - state.fade) > 0.004) {
      state.fade += (state.fadeGoal - state.fade) * interp;
      if (!state.reducedMotion) needMore = true;
    } else {
      state.fade = state.fadeGoal;
    }

    if (Math.abs(state.zFlatGoal - state.zFlat) > 0.004) {
      state.zFlat += (state.zFlatGoal - state.zFlat) * interp;
      if (!state.reducedMotion) needMore = true;
    } else {
      state.zFlat = state.zFlatGoal;
    }

    if (!state.paused && !state.reducedMotion && state.selectedIdx !== null && now < state.pulseUntil) {
      state.pulsePhase = (now * 0.003) % 6.2831853;
      needMore = true;
    } else {
      state.pulsePhase = -1.0;
    }

    if (state.pointer.down) {
      needMore = true;
    }

    renderFrame();

    if (needMore) {
      scheduleFrame();
    }
  }

  function scheduleFrame() {
    if (state.rafId !== 0 || state.destroyed || state.contextLost) return;
    state.rafId = window.requestAnimationFrame(tick);
  }

  function frameWholeGraph(instant) {
    const n = state.nNodes;
    if (n === 0) return;
    let minX = Infinity, minY = Infinity, minZ = Infinity;
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;
    for (let i = 0; i < n; i += 1) {
      const x = state.pos3d[i * 3];
      const y = state.pos3d[i * 3 + 1];
      const z = state.pos3d[i * 3 + 2];
      if (x < minX) minX = x; if (x > maxX) maxX = x;
      if (y < minY) minY = y; if (y > maxY) maxY = y;
      if (z < minZ) minZ = z; if (z > maxZ) maxZ = z;
    }
    const cx = (minX + maxX) * 0.5;
    const cy = (minY + maxY) * 0.5;
    const cz = (minZ + maxZ) * 0.5;
    const radius = Math.max(70, Math.hypot(maxX - minX, maxY - minY, maxZ - minZ) * 0.55);
    const dist = radius * 2.15;
    state.cam.txGoal = cx;
    state.cam.tyGoal = cy;
    state.cam.tzGoal = cz;
    state.cam.distGoal = dist;
    if (instant || state.reducedMotion) {
      state.cam.tx = cx; state.cam.ty = cy; state.cam.tz = cz; state.cam.dist = dist;
    }
  }

  function flyToNode(idx) {
    if (idx === null || idx < 0 || idx >= state.nNodes) return;
    const cx = state.pos3d[idx * 3];
    const cy = state.pos3d[idx * 3 + 1];
    const cz = state.pos3d[idx * 3 + 2];
    let maxR = 48;
    const nbrs = state.adj[idx];
    if (nbrs) {
      nbrs.forEach((nIdx) => {
        const d = Math.hypot(
          state.pos3d[nIdx * 3] - cx,
          state.pos3d[nIdx * 3 + 1] - cy,
          state.mode === "3d" ? state.pos3d[nIdx * 3 + 2] - cz : 0
        );
        if (d > maxR) maxR = d;
      });
    }
    const targetDist = Math.max(120, Math.min(460, maxR * 1.95));
    state.cam.txGoal = cx;
    state.cam.tyGoal = cy;
    state.cam.tzGoal = state.mode === "3d" ? cz : 0;
    state.cam.distGoal = Math.min(state.cam.distGoal, targetDist);
    if (state.reducedMotion) {
      state.cam.tx = state.cam.txGoal;
      state.cam.ty = state.cam.tyGoal;
      state.cam.tz = state.cam.tzGoal;
      state.cam.dist = state.cam.distGoal;
    }
  }

  function pickNodeAt(clientX, clientY) {
    if (!state.canvas || state.nNodes === 0) return null;
    const rect = state.canvas.getBoundingClientRect();
    const x = clientX - rect.left;
    const y = clientY - rect.top;
    const sel = state.selectedIdx;
    const nbrs = sel !== null && state.adj[sel] ? state.adj[sel] : null;
    let best = null;
    let bestScore = Infinity;
    for (let i = 0; i < state.nNodes; i += 1) {
      if (!state.screenVis[i]) continue;
      const dist = Math.hypot(state.screenX[i] - x, state.screenY[i] - y);
      const hitR = Math.max(14, state.screenR[i] + 7);
      if (dist <= hitR) {
        const prio = i === sel || (nbrs && nbrs.has(i)) ? 0.55 : 1.0;
        const score = dist * prio;
        if (score < bestScore) {
          bestScore = score;
          best = i;
        }
      }
    }
    return best;
  }

  function bindInputEvents(canvas, signal) {
    const opts = signal ? { signal } : undefined;
    canvas.addEventListener("pointerdown", (e) => {
      if (e.button !== 0 && e.button !== 2) return;
      canvas.setPointerCapture(e.pointerId);
      state.pointer.down = true;
      state.pointer.panMode = state.mode === "2d" || e.shiftKey || e.button === 2;
      state.pointer.moved = false;
      state.pointer.startX = e.clientX;
      state.pointer.startY = e.clientY;
      state.pointer.lastX = e.clientX;
      state.pointer.lastY = e.clientY;
      scheduleFrame();
    }, opts);

    canvas.addEventListener("pointermove", (e) => {
      if (state.pointer.down) {
        const dx = e.clientX - state.pointer.lastX;
        const dy = e.clientY - state.pointer.lastY;
        state.pointer.lastX = e.clientX;
        state.pointer.lastY = e.clientY;
        if (Math.hypot(e.clientX - state.pointer.startX, e.clientY - state.pointer.startY) > 4) {
          state.pointer.moved = true;
        }
        if (state.pointer.panMode || state.mode === "2d") {
          const panScale = (state.cam.dist * 0.9) / Math.max(1, state.height);
          if (state.mode === "2d") {
            state.cam.txGoal -= dx * panScale;
            state.cam.tyGoal += dy * panScale;
          } else {
            const cy = Math.cos(state.cam.yaw);
            const sy = Math.sin(state.cam.yaw);
            state.cam.txGoal -= dx * panScale * cy;
            state.cam.tzGoal += dx * panScale * sy;
            state.cam.tyGoal += dy * panScale;
          }
        } else {
          state.cam.yawGoal -= dx * 0.0065;
          state.cam.pitchGoal = Math.max(-1.35, Math.min(1.35, state.cam.pitchGoal + dy * 0.0065));
        }
        scheduleFrame();
        return;
      }

      const hovered = pickNodeAt(e.clientX, e.clientY);
      if (hovered !== state.hoverIdx) {
        state.hoverIdx = hovered;
        canvas.style.cursor = hovered !== null ? "pointer" : "grab";
        updateFocusBuffers();
        scheduleFrame();
        emit({ type: "hover", index: hovered });
      }
    }, opts);

    const endPointer = (e, isClickCandidate) => {
      if (!state.pointer.down) return;
      state.pointer.down = false;
      if (canvas.hasPointerCapture(e.pointerId)) {
        canvas.releasePointerCapture(e.pointerId);
      }
      if (isClickCandidate && !state.pointer.moved) {
        const hit = pickNodeAt(e.clientX, e.clientY);
        api.selectNode(hit);
        emit({ type: "select", index: hit });
      }
      scheduleFrame();
    };

    canvas.addEventListener("pointerup", (e) => endPointer(e, true), opts);
    canvas.addEventListener("pointercancel", (e) => endPointer(e, false), opts);
    canvas.addEventListener("contextmenu", (e) => e.preventDefault(), opts);

    canvas.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const factor = Math.exp(Math.max(-0.25, Math.min(0.25, e.deltaY * 0.0018)));
        const nextDist = Math.max(36, Math.min(3600, state.cam.distGoal * factor));
        if (state.mode === "2d") {
          const rect = canvas.getBoundingClientRect();
          const ndcX = ((e.clientX - rect.left) / Math.max(1, rect.width)) * 2 - 1;
          const ndcY = 1 - ((e.clientY - rect.top) / Math.max(1, rect.height)) * 2;
          const aspect = Math.max(0.1, rect.width / Math.max(1, rect.height));
          const oldH = state.cam.distGoal * 0.45;
          const newH = nextDist * 0.45;
          state.cam.txGoal += ndcX * aspect * (oldH - newH);
          state.cam.tyGoal += ndcY * (oldH - newH);
        }
        state.cam.distGoal = nextDist;
        scheduleFrame();
      },
      signal ? { passive: false, signal } : { passive: false }
    );
  }

  const api = {
    mount(canvasId, labelsId, opts) {
      api.destroy();
      state.destroyed = false;
      state.unavailable = false;
      state.contextLost = false;
      state.opts = Object.assign({ nativeDownload: false }, opts || {});
      state.reducedMotion = prefersReducedMotion();
      state.canvas = document.getElementById(canvasId);
      state.labelsHost = document.getElementById(labelsId);
      if (!state.canvas) return false;

      const gl = state.canvas.getContext("webgl2", {
        alpha: false,
        antialias: true,
        preserveDrawingBuffer: true,
        powerPreference: "high-performance",
      });
      if (!gl) {
        state.unavailable = true;
        emit({ type: "unavailable" });
        return false;
      }
      state.gl = gl;
      if (!initGLResources()) {
        state.unavailable = true;
        emit({ type: "unavailable" });
        return false;
      }

      state.abortController = typeof AbortController === "function" ? new AbortController() : null;
      const listenerOpts = state.abortController ? { signal: state.abortController.signal } : undefined;

      state.canvas.addEventListener("webglcontextlost", (e) => {
        e.preventDefault();
        state.contextLost = true;
        if (state.rafId) {
          window.cancelAnimationFrame(state.rafId);
          state.rafId = 0;
        }
        emit({ type: "contextlost" });
      }, listenerOpts);
      state.canvas.addEventListener("webglcontextrestored", () => {
        state.contextLost = false;
        initGLResources();
        uploadAllStaticBuffers();
        updateFocusBuffers();
        scheduleFrame();
        emit({ type: "contextrestored" });
      }, listenerOpts);

      document.addEventListener(
        "visibilitychange",
        () => {
          if (document.hidden) {
            state.pointer.down = false;
            return;
          }
          if (!state.destroyed && !state.contextLost) {
            if (state.rafId) {
              window.cancelAnimationFrame(state.rafId);
              state.rafId = 0;
            }
            resizeCanvas();
            scheduleFrame();
          }
        },
        listenerOpts
      );
      window.addEventListener(
        "blur",
        () => {
          state.pointer.down = false;
        },
        listenerOpts
      );

      bindInputEvents(state.canvas, state.abortController ? state.abortController.signal : null);
      ensureLabelPool();
      resizeCanvas();

      if (typeof ResizeObserver === "function") {
        state.resizeObserver = new ResizeObserver(scheduleResize);
        state.resizeObserver.observe(state.canvas.parentElement || state.canvas);
      }

      scheduleFrame();
      emit({ type: "ready" });
      return true;
    },

    setGraph(payload) {
      if (!payload || state.unavailable) return;
      const n = Array.isArray(payload.kinds) ? payload.kinds.length : 0;
      const e = Array.isArray(payload.edge_weights) ? payload.edge_weights.length : 0;
      state.nNodes = n;
      state.nEdges = e;
      state.kinds = Float32Array.from(payload.kinds || []);
      state.radii = Float32Array.from(payload.radii || []);
      state.degrees = Uint32Array.from(payload.degrees || []);
      state.labels = Array.isArray(payload.labels) ? payload.labels : [];
      state.topHubs = Array.isArray(payload.top_hubs) ? payload.top_hubs : [];
      state.edges = Uint32Array.from(payload.edges || []);
      state.edgeWeights = Float32Array.from(payload.edge_weights || []);

      state.adj = Array.from({ length: n }, () => new Set());
      for (let k = 0; k < e; k += 1) {
        const a = state.edges[k * 2];
        const b = state.edges[k * 2 + 1];
        if (a < n && b < n) {
          state.adj[a].add(b);
          state.adj[b].add(a);
        }
      }

      state.pos3d = Float32Array.from(payload.init_pos || []);
      state.pos = new Float32Array(state.pos3d.length);
      state.vel = new Float32Array(n * 3);
      state.fx = new Float32Array(n);
      state.fy = new Float32Array(n);
      state.fz = new Float32Array(n);
      state.focusArr = new Float32Array(n);
      state.drawOrder = new Uint32Array(n);
      state.edgePos = new Float32Array(e * 6);
      state.edgeMeta = new Float32Array(e * 6);
      state.screenX = new Float32Array(n);
      state.screenY = new Float32Array(n);
      state.screenR = new Float32Array(n);
      state.screenVis = new Uint8Array(n);
      state.selectedIdx = null;
      state.hoverIdx = null;

      let maxDeg = 0;
      for (let i = 0; i < n; i += 1) {
        if (state.degrees[i] > maxDeg) maxDeg = state.degrees[i];
      }
      state.starGraph = n >= 3 && maxDeg >= 0.7 * (n - 1);

      if (state.starGraph) {
        layoutStarGraph();
      } else {
        state.alpha = 1.0;
        syncEdgePositions();
        if (state.reducedMotion) {
          for (let s = 0; s < 64 && state.alpha > ALPHA_EPS; s += 1) {
            stepForceLayout();
          }
          state.alpha = 0;
        }
      }

      uploadAllStaticBuffers();
      updateFocusBuffers();
      frameWholeGraph(true);
      scheduleFrame();
    },

    setTheme(theme) {
      state.dark = theme === "dark";
      scheduleFrame();
    },

    setMode(mode) {
      const next = mode === "2d" ? "2d" : "3d";
      if (state.mode !== next) {
        state.mode = next;
        state.zFlatGoal = next === "2d" ? 1.0 : 0.0;
        if (state.reducedMotion) state.zFlat = state.zFlatGoal;
        if (state.selectedIdx !== null) flyToNode(state.selectedIdx);
        scheduleFrame();
      }
    },

    setLoading(loading) {
      state.loading = Boolean(loading);
      state.fadeGoal = state.loading ? 0.15 : 1.0;
      if (state.reducedMotion) state.fade = state.fadeGoal;
      scheduleFrame();
    },

    selectNode(idx) {
      const next = idx === null || idx === undefined || idx < 0 || idx >= state.nNodes ? null : idx;
      state.selectedIdx = next;
      state.pulseUntil = next !== null ? performance.now() + 900 : 0;
      updateFocusBuffers();
      if (next !== null) {
        flyToNode(next);
      }
      scheduleFrame();
    },

    zoomBy(factor) {
      if (!factor || factor <= 0) return;
      state.cam.distGoal = Math.max(36, Math.min(3600, state.cam.distGoal / factor));
      if (state.reducedMotion) state.cam.dist = state.cam.distGoal;
      scheduleFrame();
    },

    resetView() {
      state.cam.yawGoal = 0.45;
      state.cam.pitchGoal = 0.35;
      frameWholeGraph(state.reducedMotion);
      scheduleFrame();
    },

    setPaused(paused) {
      state.paused = Boolean(paused);
      if (!state.paused && state.alpha > ALPHA_EPS) {
        scheduleFrame();
      } else {
        renderFrame();
      }
    },

    exportPng() {
      if (!state.canvas || state.unavailable) return;
      renderFrame();
      const out = document.createElement("canvas");
      out.width = state.canvas.width;
      out.height = state.canvas.height;
      const ctx = out.getContext("2d");
      if (!ctx) return;
      ctx.drawImage(state.canvas, 0, 0);

      // Composite currently visible DOM labels onto snapshot
      ctx.save();
      ctx.scale(state.dpr, state.dpr);
      ctx.font = '600 12px "Segoe UI", "Helvetica Neue", sans-serif';
      ctx.textAlign = "center";
      ctx.textBaseline = "bottom";
      for (let i = 0; i < state.labelPool.length; i += 1) {
        const span = state.labelPool[i];
        if (span.style.display === "none") continue;
        const match = /translate3d\(([-\d.]+)px,\s*([-\d.]+)px/.exec(span.style.transform);
        if (!match) continue;
        const lx = parseFloat(match[1]);
        const ly = parseFloat(match[2]);
        const text = span.textContent || "";
        ctx.font = span.classList.contains("is-taxon")
          ? 'italic 600 12px "Segoe UI", "Helvetica Neue", sans-serif'
          : '600 12px "Segoe UI", "Helvetica Neue", sans-serif';
        ctx.lineWidth = 3.5;
        ctx.strokeStyle = state.dark ? "rgba(10, 15, 25, 0.92)" : "rgba(255, 255, 255, 0.94)";
        ctx.strokeText(text, lx, ly);
        ctx.fillStyle = state.dark ? "#eef4fb" : "#111827";
        ctx.fillText(text, lx, ly);
      }
      ctx.restore();

      const dataUrl = out.toDataURL("image/png");
      if (state.opts && state.opts.nativeDownload) {
        emit({ type: "png_data", dataUrl });
        return;
      }
      const a = document.createElement("a");
      a.href = dataUrl;
      a.download = "lotus-network.png";
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
    },

    destroy() {
      state.destroyed = true;
      if (state.abortController) {
        state.abortController.abort();
        state.abortController = null;
      }
      if (state.rafId) {
        window.cancelAnimationFrame(state.rafId);
        state.rafId = 0;
      }
      if (state.resizeRaf) {
        window.cancelAnimationFrame(state.resizeRaf);
        state.resizeRaf = 0;
      }
      if (state.resizeObserver) {
        state.resizeObserver.disconnect();
        state.resizeObserver = null;
      }
      if (state.canvas) {
        state.canvas.style.width = "";
        state.canvas.style.height = "";
      }
      state.width = 1;
      state.height = 1;
      state.gl = null;
      state.canvas = null;
      state.labelsHost = null;
      state.labelPool = [];
    },
  };

  window.LotusGraphGL = api;
  emit({ type: "script_loaded" });
})();
