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

  append(child: Widget) {
    this.#div.appendChild(child.getElem());
    this.#children.push(child);
  }
}
