import { Widget } from "./widget.js";

export class WidgetPage extends Widget{
  #div: HTMLDivElement;
  #child: null | Widget;

  constructor() {
    super();
    this.#div = document.createElement("div");
    this.#div.className = "widget-page";
    document.body.appendChild(this.#div);
    this.#child = null;
  }

  getElem(): HTMLElement {
    return this.#div;
  }

  setChild(child: Widget) {
    while (this.#div.childNodes.length > 0) {
      this.#div.removeChild(this.#div.childNodes[0]);
    }
    this.#div.appendChild(child.getElem());
    this.#child = child;
  }

  getChild(): Widget | null {
    return this.#child;
  }
}
