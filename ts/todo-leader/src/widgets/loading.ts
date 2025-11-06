import { Widget } from "./widget.js";

export class WidgetLoading extends Widget {
  #div: HTMLDivElement;

  constructor() {
    super();
    this.#div = document.createElement("div");
    this.#div.className = "widget-loading";
    this.#div.appendChild(document.createTextNode("loading..."));
  }

  getElem(): HTMLElement {
    return this.#div;
  }
}
