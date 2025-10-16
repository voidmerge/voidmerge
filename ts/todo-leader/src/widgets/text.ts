import { Widget } from "./widget.js";

export class WidgetText extends Widget {
  #input: HTMLInputElement;

  constructor(value?: string) {
    super();
    this.#input = document.createElement("input");
    this.#input.type = "text";
    this.#input.className = "widget-input";
    this.set(value || "");
  }

  set(value: string) {
    this.#input.value = value;
  }

  getElem(): HTMLElement {
    return this.#input;
  }
}
