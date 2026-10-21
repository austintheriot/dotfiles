import {RECORDER_REFUSED_EXIT} from '../video/recording-state'
import {VmError} from './proc'

export function stopFailure(vm: string, code: number): VmError {
  if (code === RECORDER_REFUSED_EXIT) return new VmError('', code)
  return new VmError(`the recorder in ${vm} did not stop cleanly`, code)
}
