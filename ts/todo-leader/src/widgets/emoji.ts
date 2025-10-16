import { Widget } from "./widget.js";

export const Emoji = {
  star: "⭐",
  circleBlack: "⚫",
  hamburger: "🍔",
  pencil: "✏️",
  check: "✅",
};

export class WidgetEmoji extends Widget {
  #div: HTMLDivElement;

  constructor(emoji?: string) {
    super();
    this.#div = document.createElement("div");
    this.#div.className = "widget-emoji";
    if (emoji) {
      this.set(emoji);
    }
  }

  getElem(): HTMLElement {
    return this.#div;
  }

  set(emoji: string) {
    while (this.#div.childNodes.length > 0) {
      this.#div.removeChild(this.#div.childNodes[0]);
    }
    this.#div.appendChild(document.createTextNode(emoji));
  }
}
