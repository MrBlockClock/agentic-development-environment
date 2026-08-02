import type { ReactNode } from "react";
import type { GettingStartedWin } from "./GettingStartedChecklist";

export type GettingStartedInput = {
  understand: boolean;
  verify: boolean;
  improveAde: boolean;
  isDogfood: boolean;
  understandBusy?: boolean;
  verifying?: boolean;
  improveBusy?: boolean;
  onUnderstand: () => void;
  onVerify: () => void;
  onImprove?: () => void;
  keysTrailing?: ReactNode;
  improveTrailing?: ReactNode;
};

/** Single source for Home (browser) and Agent empty-canvas Getting started rows. */
export function buildGettingStartedSteps(
  input: GettingStartedInput,
): GettingStartedWin[] {
  const steps: GettingStartedWin[] = [
    {
      id: "understand",
      title: "Learn this project",
      detail: "Write a short project snapshot you can reuse",
      done: input.understand,
      busy: input.understandBusy,
      onClick: input.onUnderstand,
    },
    {
      id: "verify",
      title: "Check that things still work",
      detail: "Run ADE’s built-in checks on this workspace",
      done: input.verify,
      busy: input.verifying,
      onClick: input.onVerify,
      trailing: input.keysTrailing,
    },
  ];
  if (input.isDogfood && input.onImprove) {
    steps.push({
      id: "improve",
      title: "Try a small safe change",
      detail: "Open Agent with a careful, check-after change",
      done: input.improveAde,
      busy: input.improveBusy,
      onClick: input.onImprove,
      trailing: input.improveTrailing,
    });
  }
  return steps;
}
