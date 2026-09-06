import { Injectable } from '@nestjs/common';

@Injectable()
export class UserRepository {
  find() { return ['fixture-user']; }
}
