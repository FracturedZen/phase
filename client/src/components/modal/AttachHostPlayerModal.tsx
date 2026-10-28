import { useTranslation } from "react-i18next";

import type { GameAction, PlayerId, WaitingFor } from "../../adapter/types.ts";
import { useGameDispatch } from "../../hooks/useGameDispatch.ts";
import { useCanActForWaitingState } from "../../hooks/usePlayerId.ts";
import { useGameStore } from "../../stores/gameStore.ts";
import { getOpponentDisplayName } from "../../stores/multiplayerStore.ts";
import { ChoiceModal } from "./ChoiceModal.tsx";

type AttachHostPlayerWaitingFor = Extract<WaitingFor, { type: "AttachHostPlayerChoice" }>;

interface AttachHostPlayerModalContentProps {
  waitingFor: AttachHostPlayerWaitingFor;
  seatOrder?: PlayerId[];
  dispatch: (action: GameAction) => void | Promise<void>;
}

/**
 * CR 608.2d + CR 303.4: a resolving attach chooses which player receives the
 * already-bound attachments. The engine offers only legal hosts.
 */
export function AttachHostPlayerModalContent({
  waitingFor,
  seatOrder,
  dispatch,
}: AttachHostPlayerModalContentProps) {
  const { t } = useTranslation("game");
  const candidates = [...waitingFor.data.candidates].sort((a, b) => {
    const aIdx = seatOrder?.indexOf(a) ?? a;
    const bIdx = seatOrder?.indexOf(b) ?? b;
    return aIdx - bIdx;
  });

  return (
    <ChoiceModal
      title={t("attachHostPlayer.title")}
      subtitle={t("attachHostPlayer.subtitle")}
      options={candidates.map((opponent) => ({
        id: String(opponent),
        label: getOpponentDisplayName(opponent),
      }))}
      onChoose={(id) => {
        dispatch({
          type: "ChooseAttachHostPlayer",
          data: { opponent: Number(id) },
        });
      }}
    />
  );
}

export function AttachHostPlayerModal() {
  const canActForWaitingState = useCanActForWaitingState();
  const dispatch = useGameDispatch();
  const waitingFor = useGameStore((s) => s.waitingFor);
  const seatOrder = useGameStore((s) => s.gameState?.seat_order);

  if (waitingFor?.type !== "AttachHostPlayerChoice") return null;
  if (!canActForWaitingState) return null;

  return (
    <AttachHostPlayerModalContent
      waitingFor={waitingFor}
      seatOrder={seatOrder}
      dispatch={dispatch}
    />
  );
}
