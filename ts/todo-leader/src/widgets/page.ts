import { Widget } from "./widget.js";

export class WidgetPage extends Widget {
  #divPage: HTMLDivElement;
  #divFrame: HTMLDivElement;
  #child: null | Widget;

  constructor() {
    super();
    this.#divPage = document.createElement("div");
    this.#divPage.className = "widget-page";
    document.body.appendChild(this.#divPage);
    this.#divFrame = document.createElement("div");
    this.#divFrame.className = "widget-frame";
    this.#divPage.appendChild(this.#divFrame);
    this.#child = null;
  }

  getElem(): HTMLElement {
    return this.#divPage;
  }

  setChild(child: Widget) {
    while (this.#divFrame.childNodes.length > 0) {
      this.#divFrame.removeChild(this.#divFrame.childNodes[0]);
    }
    this.#divFrame.appendChild(child.getElem());
    this.#child = child;
  }

  getChild(): Widget | null {
    return this.#child;
  }
}
