import { Widget } from "./widget.js";

export class WidgetLabel extends Widget {
  #div: HTMLDivElement;
  #val: string;

  constructor(value?: string) {
    super();
    this.#div = document.createElement("div");
    this.#div.className = "widget-label";
    this.#val = "";
    this.set(value || "");
  }

  get(): string {
    return this.#val;
  }

  set(text: string) {
    if (this.#val !== text) {
      while (this.#div.childNodes.length > 0) {
        this.#div.removeChild(this.#div.childNodes[0]);
      }
      this.#div.appendChild(document.createTextNode(text));
    }
  }

  getElem(): HTMLElement {
    return this.#div;
  }
}
