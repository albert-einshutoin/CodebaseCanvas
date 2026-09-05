import { Injectable } from '@nestjs/common';
import { UserRepository } from './user.repository';

@Injectable()
export class UsersService {
  constructor(private readonly users: UserRepository) {}
  list() { return this.users.find(); }
  static find() { return 'static'; }
  find() { return 'instance'; }
  choose(flag: boolean) {
    this.find();
    this[flag ? 'find' : 'list']();
    const nested = () => this.find();
    return nested;
  }
}
