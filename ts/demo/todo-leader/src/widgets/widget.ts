export class Widget {
  getElem(): HTMLElement {
    throw new Error("subclasses must implement getElem");
  }

  handleClick(cb: () => void) {
    this.getElem().onclick = (evt) => {
      evt.preventDefault();
      cb();
    };
  }

  remove() {
    const elem = this.getElem();
    if (elem.parentNode) {
      elem.parentNode.removeChild(elem);
    }
  }
}
