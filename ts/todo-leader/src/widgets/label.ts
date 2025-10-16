import { Widget } from "./widget.js";

export class WidgetLabel extends Widget {
  #div: HTMLDivElement;

  constructor(value?: string) {
    super();
    this.#div = document.createElement("div");
    this.#div.className = "widget-label";
    this.#div.appendChild(document.createTextNode(value || ""));
  }

  getElem(): HTMLElement {
    return this.#div;
  }
}
