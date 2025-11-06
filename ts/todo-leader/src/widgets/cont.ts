import { Widget } from "./widget.js";

export class WidgetCont extends Widget {
  #div: HTMLDivElement;
  #children: Widget[];

  constructor() {
    super();
    this.#div = document.createElement("div");
    this.#children = [];
  }

  getElem(): HTMLElement {
    return this.#div;
  }

  clear() {
    while (this.#div.childNodes.length > 0) {
      this.#div.removeChild(this.#div.childNodes[0]);
    }
    this.#children = [];
  }

  append(child: Widget) {
    this.#div.appendChild(child.getElem());
    this.#children.push(child);
  }
}
