import { Widget } from "./widget.js";

export class WidgetText extends Widget {
  #update: () => void;
  #input: HTMLInputElement;

  constructor(value?: string) {
    super();
    this.#update = () => {};
    this.#input = document.createElement("input");
    this.#input.type = "text";
    this.#input.className = "widget-input";
    this.#input.oninput = () => {
      this.#update();
    };
    this.set(value || "");
  }

  setUpdate(update: () => void) {
    this.#update = update;
  }

  get(): string {
    return this.#input.value;
  }

  set(value: string) {
    if (this.#input.value !== value) {
      this.#input.value = value;
    }
  }

  getElem(): HTMLElement {
    return this.#input;
  }
}
