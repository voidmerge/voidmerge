import { Widget } from "./widget.js";

export const Emoji = {
  back: "🔙",
  star: "⭐",
  circleBlack: "⚫",
  check: "✅",
  trophy: "🏆",
  calendar: "🗓️",
};

export class WidgetEmoji extends Widget {
  #state: string;
  #div: HTMLDivElement;

  constructor(emoji?: string) {
    super();
    this.#state = "";
    this.#div = document.createElement("div");
    this.#div.className = "widget-emoji";
    if (emoji) {
      this.set(emoji);
    }
  }

  getElem(): HTMLElement {
    return this.#div;
  }

  get(): string {
    return this.#state;
  }

  set(emoji: string) {
    this.#state = emoji;
    while (this.#div.childNodes.length > 0) {
      this.#div.removeChild(this.#div.childNodes[0]);
    }
    this.#div.appendChild(document.createTextNode(emoji));
  }
}
