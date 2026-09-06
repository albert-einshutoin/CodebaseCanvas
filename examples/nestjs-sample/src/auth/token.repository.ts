import { Injectable } from '@nestjs/common';

@Injectable()
export class TokenRepository {
  save() { return 'fixture-token'; }
}
