import { WidgetCont } from "./cont.js";

export class WidgetHoriz extends WidgetCont {
  constructor() {
    super();
    this.getElem().className = "widget-horiz expand";
  }

  setWrap(v: boolean) {
    this.getElem().classList.toggle("wrap", v);
  }

  setExpand(v: boolean) {
    this.getElem().classList.toggle("expand", v);
  }
}
