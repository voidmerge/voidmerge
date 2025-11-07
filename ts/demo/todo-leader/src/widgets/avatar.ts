import { Widget } from "./widget.js";

export class WidgetAvatar extends Widget {
  #img: HTMLImageElement;

  constructor(avatarCode: string) {
    super();
    this.#img = document.createElement("img");
    this.#img.className = "widget-avatar";
    this.setAvatarCode(avatarCode);
  }

  setAvatarCode(avatarCode: string) {
    this.#img.src = `avatar/${avatarCode}`;
  }

  getElem(): HTMLElement {
    return this.#img;
  }
}
