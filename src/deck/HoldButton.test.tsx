import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { HoldButton } from './HoldButton';

// jsdom has no pointer capture.
beforeAll(() => {
  Element.prototype.setPointerCapture = () => {};
});
afterEach(cleanup);

function setup() {
  const onHold = vi.fn();
  const view = render(
    <HoldButton className="b" label="Halten" onHold={onHold}>
      X
    </HoldButton>,
  );
  return { onHold, button: view.getByRole('button', { name: 'Halten' }), view };
}

describe('HoldButton', () => {
  it('holds while the primary button is down', () => {
    const { onHold, button } = setup();
    fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
    fireEvent.pointerUp(button, { button: 0, pointerId: 1 });
    expect(onHold.mock.calls).toEqual([[true], [false]]);
  });

  it('ignores the right mouse button', () => {
    const { onHold, button } = setup();
    fireEvent.pointerDown(button, { button: 2, pointerId: 1 });
    expect(onHold).not.toHaveBeenCalled();
  });

  it('releases on unmount while held', () => {
    const { onHold, button, view } = setup();
    fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
    view.unmount();
    expect(onHold.mock.calls).toEqual([[true], [false]]);
  });

  it('works from the keyboard and never sticks on blur', () => {
    const { onHold, button } = setup();
    fireEvent.keyDown(button, { key: ' ' });
    fireEvent.blur(button);
    fireEvent.keyUp(button, { key: ' ' });
    expect(onHold.mock.calls).toEqual([[true], [false]]);
  });
});
