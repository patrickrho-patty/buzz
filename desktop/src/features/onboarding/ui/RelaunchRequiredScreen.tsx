import { RecoveryScreen } from "./RecoveryScreen";

export function RelaunchRequiredScreen() {
  return (
    <RecoveryScreen
      testId="relaunch-required"
      title="Restart Crew to finish recovery"
      body="Your identity was updated. Crew needs to restart so syncing and agents run under it."
    />
  );
}
