import { map as rootMap } from 'rxjs';
import { map as operatorsMap } from 'rxjs/operators';

export namespace Alpha { export class Same {} }
export namespace Beta { export class Same {} }
export class ExternalBindings {
  inspect() { return [rootMap, operatorsMap]; }
}
