// The right-hand detail panel: title, status pill, big value, sparkline, meta.
// main.js calls open(id) on hotspot click and refresh(snapshot) on every tick.

export function createDrawer({ data }) {
  const el = document.getElementById("drawer");
  const title = document.getElementById("drawer-title");
  const status = document.getElementById("drawer-status");
  const value = document.getElementById("drawer-value");
  const meta = document.getElementById("drawer-meta");
  const spark = document.getElementById("spark");

  let openId = null;

  document.getElementById("drawer-close").addEventListener("click", close);

  function open(id) {
    openId = id;
    el.classList.remove("hidden");
    render();
  }

  function close() {
    openId = null;
    el.classList.add("hidden");
  }

  // called on every data tick — repaint if something is open
  function refresh() {
    if (openId) render();
  }

  function render() {
    const a = data.assets().find((x) => x.id === openId);
    if (!a) return close();
    title.textContent = a.label;
    status.textContent = a.status;
    status.className = `pill ${a.status}`;
    value.textContent = `${a.value.toFixed(1)} ${a.unit}`;
    const hist = data.history(a.id);
    drawSpark(spark, hist, a.status);
    meta.innerHTML = `
      <li><span>Kind</span>${a.kind}</li>
      <li><span>Points</span>${hist.length}</li>
      <li><span>Min</span>${Math.min(...hist).toFixed(1)} ${a.unit}</li>
      <li><span>Max</span>${Math.max(...hist).toFixed(1)} ${a.unit}</li>`;
  }

  return { open, close, refresh };
}

const COLOR = { normal: "#23c268", warning: "#f2b23c", critical: "#f0483e" };

function drawSpark(canvas, hist, statusKey) {
  const ctx = canvas.getContext("2d");
  const dpr = Math.min(devicePixelRatio, 2);
  const w = canvas.clientWidth || 300, h = canvas.clientHeight || 90;
  canvas.width = w * dpr;
  canvas.height = h * dpr;
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, w, h);
  if (hist.length < 2) return;

  const min = Math.min(...hist), max = Math.max(...hist);
  const span = max - min || 1;
  const x = (i) => (i / (hist.length - 1)) * (w - 8) + 4;
  const y = (v) => h - 8 - ((v - min) / span) * (h - 16);
  const color = COLOR[statusKey] || COLOR.normal;

  // soft fill under the line
  ctx.beginPath();
  ctx.moveTo(x(0), h);
  hist.forEach((v, i) => ctx.lineTo(x(i), y(v)));
  ctx.lineTo(x(hist.length - 1), h);
  ctx.closePath();
  ctx.fillStyle = color + "22";
  ctx.fill();

  ctx.beginPath();
  hist.forEach((v, i) => (i ? ctx.lineTo(x(i), y(v)) : ctx.moveTo(x(i), y(v))));
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.6;
  ctx.stroke();

  // dot on the latest point
  ctx.beginPath();
  ctx.arc(x(hist.length - 1), y(hist[hist.length - 1]), 2.6, 0, Math.PI * 2);
  ctx.fillStyle = color;
  ctx.fill();
}
