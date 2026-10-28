import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { GameAction, WaitingFor } from "../../../adapter/types.ts";
import { isWaitingForHandled } from "../../../game/waitingForRegistry.ts";
import { useMultiplayerStore } from "../../../stores/multiplayerStore.ts";
import { AttachHostPlayerModalContent } from "../AttachHostPlayerModal.tsx";

type AttachHostWaitingFor = Extract<WaitingFor, { type: "AttachHostPlayerChoice" }>;

function attachHostWaitingFor(): AttachHostWaitingFor {
  return {
    type: "AttachHostPlayerChoice",
    data: {
      player: 0,
      candidates: [2, 1],
      attachments: [10],
      source_id: 4,
    },
  };
}

function renderModal(waitingFor: AttachHostWaitingFor) {
  const dispatch = vi.fn<(action: GameAction) => void>();
  render(
    <AttachHostPlayerModalContent
      waitingFor={waitingFor}
      seatOrder={[0, 1, 2]}
      dispatch={dispatch}
    />,
  );
  return dispatch;
}

afterEach(() => {
  cleanup();
  useMultiplayerStore.setState({ playerNames: new Map() });
});

describe("AttachHostPlayerModalContent", () => {
  it("registers the waiting state as handled", () => {
    expect(isWaitingForHandled(attachHostWaitingFor())).toBe(true);
  });

  it("dispatches the selected player host", () => {
    useMultiplayerStore.setState({
      playerNames: new Map([
        [1, "Alice"],
        [2, "Bob"],
      ]),
    });
    const dispatch = renderModal(attachHostWaitingFor());

    expect(screen.getByRole("heading", { name: "Choose a Player" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Bob" }));

    expect(dispatch).toHaveBeenCalledWith({
      type: "ChooseAttachHostPlayer",
      data: { opponent: 2 },
    });
  });
});
