import { Stage } from "./core/Stage";
import { productIndex, createProduct } from "./builder/index";
import type { Product } from "./core/Product";

const productList = document.getElementById("product-list")!;
const infoTitle = document.getElementById("info-title")!;
const infoList = document.getElementById("info-list")!;
const toggleExplode = document.getElementById("toggle-explode") as HTMLInputElement;
const toggleEnclosure = document.getElementById("toggle-enclosure") as HTMLInputElement;
const toggleLabels = document.getElementById("toggle-labels") as HTMLInputElement;
const hint = document.getElementById("hint")!;

const stage = new Stage(document.getElementById("stage") as HTMLCanvasElement);

let active: Product | null = null;

function loadProduct(name: string): void {
  const product = createProduct(name);
  if (!product) return;
  product.build();
  stage.setProduct(product);
  active = product;
  updatePanelButtons(name);
  infoTitle.textContent = product.group.name;
  infoList.innerHTML = "";
  applyToggles();
}

function updatePanelButtons(activeName: string): void {
  productList.innerHTML = "";
  for (const entry of productIndex) {
    const btn = document.createElement("button");
    btn.textContent = entry.name;
    btn.title = entry.description;
    btn.classList.toggle("active", entry.name === activeName);
    btn.addEventListener("click", () => loadProduct(entry.name));
    productList.appendChild(btn);
  }
}

stage.onSelection = (sel) => {
  infoTitle.textContent = sel ? sel.label : active?.group.name ?? "—";
  infoList.innerHTML = "";
  if (sel?.meta) {
    for (const [k, v] of Object.entries(sel.meta)) {
      const li = document.createElement("li");
      li.innerHTML = `<span>${k}</span><b>${String(v)}</b>`;
      infoList.appendChild(li);
    }
  }
};

function applyToggles(): void {
  if (!active) return;
  const explode = Number(toggleExplode.value) / 100;
  active.applyExplode(explode);

  active.setVisible((p) => {
    if (!toggleEnclosure.checked && p.label === "Enclosure") return false;
    if (!toggleLabels.checked && p.object.userData.label) return false;
    return true;
  });
}

toggleExplode.addEventListener("input", applyToggles);
toggleEnclosure.addEventListener("change", applyToggles);
toggleLabels.addEventListener("change", applyToggles);

setTimeout(() => hint.classList.add("fade"), 8000);

loadProduct("LB-MINI-BMS");
stage.start();